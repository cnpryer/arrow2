use std::{collections::VecDeque, sync::Arc};

use parquet2::{
    encoding::hybrid_rle::HybridRleDecoder, page::DataPage, read::levels::get_bit_width,
};

use crate::{array::Array, bitmap::MutableBitmap, error::Result};

use super::super::DataPages;
use super::utils::{split_buffer, DecodedState, Decoder, MaybeNext, Pushable};

/// trait describing deserialized repetition and definition levels
pub trait Nested: std::fmt::Debug + Send + Sync {
    fn inner(&mut self) -> (Vec<i64>, Option<MutableBitmap>);

    fn push(&mut self, length: i64, is_valid: bool);

    fn is_nullable(&self) -> bool;

    /// number of rows
    fn len(&self) -> usize;

    /// number of values associated to the primitive type this nested tracks
    fn num_values(&self) -> usize;
}

#[derive(Debug, Default)]
pub struct NestedPrimitive {
    is_nullable: bool,
    length: usize,
}

impl NestedPrimitive {
    pub fn new(is_nullable: bool) -> Self {
        Self {
            is_nullable,
            length: 0,
        }
    }
}

impl Nested for NestedPrimitive {
    fn inner(&mut self) -> (Vec<i64>, Option<MutableBitmap>) {
        (Default::default(), Default::default())
    }

    fn is_nullable(&self) -> bool {
        self.is_nullable
    }

    fn push(&mut self, _value: i64, _is_valid: bool) {
        self.length += 1
    }

    fn len(&self) -> usize {
        self.length
    }

    fn num_values(&self) -> usize {
        self.length
    }
}

#[derive(Debug, Default)]
pub struct NestedOptional {
    pub validity: MutableBitmap,
    pub offsets: Vec<i64>,
}

impl Nested for NestedOptional {
    fn inner(&mut self) -> (Vec<i64>, Option<MutableBitmap>) {
        let offsets = std::mem::take(&mut self.offsets);
        let validity = std::mem::take(&mut self.validity);
        (offsets, Some(validity))
    }

    fn is_nullable(&self) -> bool {
        true
    }

    fn push(&mut self, value: i64, is_valid: bool) {
        self.offsets.push(value);
        self.validity.push(is_valid);
    }

    fn len(&self) -> usize {
        self.offsets.len()
    }

    fn num_values(&self) -> usize {
        self.offsets.last().copied().unwrap_or(0) as usize
    }
}

impl NestedOptional {
    pub fn with_capacity(capacity: usize) -> Self {
        let offsets = Vec::<i64>::with_capacity(capacity + 1);
        let validity = MutableBitmap::with_capacity(capacity);
        Self { validity, offsets }
    }
}

#[derive(Debug, Default)]
pub struct NestedValid {
    pub offsets: Vec<i64>,
}

impl Nested for NestedValid {
    fn inner(&mut self) -> (Vec<i64>, Option<MutableBitmap>) {
        let offsets = std::mem::take(&mut self.offsets);
        (offsets, None)
    }

    fn is_nullable(&self) -> bool {
        false
    }

    fn push(&mut self, value: i64, _is_valid: bool) {
        self.offsets.push(value);
    }

    fn len(&self) -> usize {
        self.offsets.len()
    }

    fn num_values(&self) -> usize {
        self.offsets.last().copied().unwrap_or(0) as usize
    }
}

impl NestedValid {
    pub fn with_capacity(capacity: usize) -> Self {
        let offsets = Vec::<i64>::with_capacity(capacity + 1);
        Self { offsets }
    }
}

#[derive(Debug, Default)]
pub struct NestedStructValid {
    length: usize,
}

impl NestedStructValid {
    pub fn new() -> Self {
        Self { length: 0 }
    }
}

impl Nested for NestedStructValid {
    fn inner(&mut self) -> (Vec<i64>, Option<MutableBitmap>) {
        (Default::default(), None)
    }

    fn is_nullable(&self) -> bool {
        false
    }

    fn push(&mut self, _value: i64, _is_valid: bool) {
        self.length += 1;
    }

    fn len(&self) -> usize {
        self.length
    }

    fn num_values(&self) -> usize {
        self.length
    }
}

#[derive(Debug, Default)]
pub struct NestedStruct {
    validity: MutableBitmap,
}

impl NestedStruct {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            validity: MutableBitmap::with_capacity(capacity),
        }
    }
}

impl Nested for NestedStruct {
    fn inner(&mut self) -> (Vec<i64>, Option<MutableBitmap>) {
        (Default::default(), None)
    }

    fn is_nullable(&self) -> bool {
        false
    }

    fn push(&mut self, _value: i64, is_valid: bool) {
        self.validity.push(is_valid)
    }

    fn len(&self) -> usize {
        self.validity.len()
    }

    fn num_values(&self) -> usize {
        self.validity.len()
    }
}

pub(super) fn read_optional_values<D, C, G, P>(
    def_levels: D,
    max_def: u32,
    mut new_values: G,
    values: &mut P,
    validity: &mut MutableBitmap,
    mut remaining: usize,
) where
    D: Iterator<Item = u32>,
    G: Iterator<Item = C>,
    C: Default,
    P: Pushable<C>,
{
    for def in def_levels {
        if def == max_def {
            values.push(new_values.next().unwrap());
            validity.push(true);
            remaining -= 1;
        } else if def == max_def - 1 {
            values.push(C::default());
            validity.push(false);
            remaining -= 1;
        }
        if remaining == 0 {
            break;
        }
    }
}

#[derive(Debug, Clone)]
pub enum InitNested {
    Primitive(bool),
    List(Box<InitNested>, bool),
    Struct(Box<InitNested>, bool),
}

impl InitNested {
    pub fn is_primitive(&self) -> bool {
        matches!(self, Self::Primitive(_))
    }
}

fn init_nested_recursive(init: &InitNested, capacity: usize, container: &mut Vec<Box<dyn Nested>>) {
    match init {
        InitNested::Primitive(is_nullable) => {
            container.push(Box::new(NestedPrimitive::new(*is_nullable)) as Box<dyn Nested>)
        }
        InitNested::List(inner, is_nullable) => {
            container.push(if *is_nullable {
                Box::new(NestedOptional::with_capacity(capacity)) as Box<dyn Nested>
            } else {
                Box::new(NestedValid::with_capacity(capacity)) as Box<dyn Nested>
            });
            init_nested_recursive(inner, capacity, container)
        }
        InitNested::Struct(inner, is_nullable) => {
            if *is_nullable {
                container.push(Box::new(NestedStruct::with_capacity(capacity)) as Box<dyn Nested>)
            } else {
                container.push(Box::new(NestedStructValid::new()) as Box<dyn Nested>)
            }
            init_nested_recursive(inner, capacity, container)
        }
    }
}

fn init_nested(init: &InitNested, capacity: usize) -> NestedState {
    let mut container = vec![];
    init_nested_recursive(init, capacity, &mut container);
    NestedState::new(container)
}

pub struct NestedPage<'a> {
    iter: std::iter::Peekable<std::iter::Zip<HybridRleDecoder<'a>, HybridRleDecoder<'a>>>,
}

impl<'a> NestedPage<'a> {
    pub fn new(page: &'a DataPage) -> Self {
        let (rep_levels, def_levels, _) = split_buffer(page);

        let max_rep_level = page.descriptor().max_rep_level();
        let max_def_level = page.descriptor().max_def_level();

        let reps =
            HybridRleDecoder::new(rep_levels, get_bit_width(max_rep_level), page.num_values());
        let defs =
            HybridRleDecoder::new(def_levels, get_bit_width(max_def_level), page.num_values());

        let iter = reps.zip(defs).peekable();

        Self { iter }
    }

    // number of values (!= number of rows)
    pub fn len(&self) -> usize {
        self.iter.size_hint().0
    }
}

#[derive(Debug)]
pub struct NestedState {
    pub nested: Vec<Box<dyn Nested>>,
}

impl NestedState {
    pub fn new(nested: Vec<Box<dyn Nested>>) -> Self {
        Self { nested }
    }

    /// The number of rows in this state
    pub fn len(&self) -> usize {
        // outermost is the number of rows
        self.nested[0].len()
    }

    /// The number of values associated with the primitive type
    pub fn num_values(&self) -> usize {
        self.nested.last().unwrap().num_values()
    }
}

pub(super) fn extend_from_new_page<'a, T: Decoder<'a>>(
    mut page: T::State,
    items: &mut VecDeque<T::DecodedState>,
    nested: &VecDeque<NestedState>,
    decoder: &T,
) {
    let needed = nested.back().unwrap().num_values();

    let mut decoded = if let Some(decoded) = items.pop_back() {
        // there is a already a state => it must be incomplete...
        debug_assert!(
            decoded.len() < needed,
            "the temp page is expected to be incomplete ({} < {})",
            decoded.len(),
            needed
        );
        decoded
    } else {
        // there is no state => initialize it
        decoder.with_capacity(needed)
    };

    let remaining = needed - decoded.len();

    // extend the current state
    decoder.extend_from_state(&mut page, &mut decoded, remaining);

    // the number of values required is always fulfilled because
    // dremel assigns one (rep, def) to each value and we request
    // items that complete a row
    assert_eq!(decoded.len(), needed);

    items.push_back(decoded);

    for nest in nested.iter().skip(1) {
        let num_values = nest.num_values();
        let mut decoded = decoder.with_capacity(num_values);
        decoder.extend_from_state(&mut page, &mut decoded, num_values);
        items.push_back(decoded);
    }
}

/// Extends `state` by consuming `page`, optionally extending `items` if `page`
/// has less items than `chunk_size`
pub fn extend_offsets1<'a>(
    page: &mut NestedPage<'a>,
    init: &InitNested,
    items: &mut VecDeque<NestedState>,
    chunk_size: usize,
) {
    let mut nested = if let Some(nested) = items.pop_back() {
        // there is a already a state => it must be incomplete...
        debug_assert!(
            nested.len() < chunk_size,
            "the temp array is expected to be incomplete"
        );
        nested
    } else {
        // there is no state => initialize it
        init_nested(init, chunk_size)
    };

    let remaining = chunk_size - nested.len();

    // extend the current state
    extend_offsets2(page, &mut nested, remaining);
    items.push_back(nested);

    while page.len() > 0 {
        let mut nested = init_nested(init, chunk_size);
        extend_offsets2(page, &mut nested, chunk_size);
        items.push_back(nested);
    }
}

fn extend_offsets2<'a>(page: &mut NestedPage<'a>, nested: &mut NestedState, additional: usize) {
    let nested = &mut nested.nested;
    let mut values_count = vec![0; nested.len()];

    for (depth, nest) in nested.iter().enumerate().skip(1) {
        values_count[depth - 1] = nest.len() as i64
    }
    values_count[nested.len() - 1] = nested[nested.len() - 1].len() as i64;

    let mut cum_sum = vec![0u32; nested.len() + 1];
    for (i, nest) in nested.iter().enumerate() {
        let delta = if nest.is_nullable() { 2 } else { 1 };
        cum_sum[i + 1] = cum_sum[i] + delta;
    }

    let mut rows = 0;
    while let Some((rep, def)) = page.iter.next() {
        if rep == 0 {
            rows += 1;
        }

        for (depth, (nest, length)) in nested.iter_mut().zip(values_count.iter()).enumerate() {
            if depth as u32 >= rep && def >= cum_sum[depth] {
                let is_valid = nest.is_nullable() && def != cum_sum[depth];
                nest.push(*length, is_valid)
            }
        }

        for (depth, nest) in nested.iter().enumerate().skip(1) {
            values_count[depth - 1] = nest.len() as i64
        }
        values_count[nested.len() - 1] = nested[nested.len() - 1].len() as i64;

        let next_rep = page.iter.peek().map(|x| x.0).unwrap_or(0);

        if next_rep == 0 && rows == additional + 1 {
            break;
        }
    }
}

// The state of an optional DataPage with a boolean physical type
#[derive(Debug)]
pub struct Optional<'a> {
    pub definition_levels: HybridRleDecoder<'a>,
    max_def: u32,
}

impl<'a> Optional<'a> {
    pub fn new(page: &'a DataPage) -> Self {
        let (_, def_levels, _) = split_buffer(page);

        let max_def = page.descriptor().max_def_level();

        Self {
            definition_levels: HybridRleDecoder::new(
                def_levels,
                get_bit_width(max_def),
                page.num_values(),
            ),
            max_def: max_def as u32,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.definition_levels.size_hint().0
    }

    #[inline]
    pub fn max_def(&self) -> u32 {
        self.max_def
    }
}

#[inline]
pub(super) fn next<'a, I, D>(
    iter: &'a mut I,
    items: &mut VecDeque<D::DecodedState>,
    nested_items: &mut VecDeque<NestedState>,
    init: &InitNested,
    chunk_size: usize,
    decoder: &D,
) -> MaybeNext<Result<(NestedState, D::DecodedState)>>
where
    I: DataPages,
    D: Decoder<'a>,
{
    // front[a1, a2, a3, ...]back
    if items.len() > 1 {
        let nested = nested_items.pop_front().unwrap();
        let decoded = items.pop_front().unwrap();
        return MaybeNext::Some(Ok((nested, decoded)));
    }
    match iter.next() {
        Err(e) => MaybeNext::Some(Err(e.into())),
        Ok(None) => {
            if let Some(nested) = nested_items.pop_front() {
                // we have a populated item and no more pages
                // the only case where an item's length may be smaller than chunk_size
                let decoded = items.pop_front().unwrap();
                MaybeNext::Some(Ok((nested, decoded)))
            } else {
                MaybeNext::None
            }
        }
        Ok(Some(page)) => {
            // there is a new page => consume the page from the start
            let mut nested_page = NestedPage::new(page);

            extend_offsets1(&mut nested_page, init, nested_items, chunk_size);

            let maybe_page = decoder.build_state(page);
            let page = match maybe_page {
                Ok(page) => page,
                Err(e) => return MaybeNext::Some(Err(e)),
            };

            extend_from_new_page(page, items, nested_items, decoder);

            if nested_items.front().unwrap().len() < chunk_size {
                MaybeNext::More
            } else {
                let nested = nested_items.pop_front().unwrap();
                let decoded = items.pop_front().unwrap();
                MaybeNext::Some(Ok((nested, decoded)))
            }
        }
    }
}

pub type NestedArrayIter<'a> =
    Box<dyn Iterator<Item = Result<(NestedState, Arc<dyn Array>)>> + Send + Sync + 'a>;
