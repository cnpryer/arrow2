use std::collections::VecDeque;
use std::{
    io::{Read, Seek},
    sync::Arc,
};

use arrow_format::ipc::BodyCompressionRef;
use arrow_format::ipc::MetadataVersion;

use crate::array::*;
use crate::datatypes::{DataType, Field, PhysicalType};
use crate::error::Result;
use crate::io::ipc::IpcField;

use super::{array::*, Dictionaries};
use super::{IpcBuffer, Node};

#[allow(clippy::too_many_arguments)]
pub fn read<R: Read + Seek>(
    field_nodes: &mut VecDeque<Node>,
    field: &Field,
    ipc_field: &IpcField,
    buffers: &mut VecDeque<IpcBuffer>,
    reader: &mut R,
    dictionaries: &Dictionaries,
    block_offset: u64,
    is_little_endian: bool,
    compression: Option<BodyCompressionRef>,
    version: MetadataVersion,
) -> Result<Arc<dyn Array>> {
    use PhysicalType::*;
    let data_type = field.data_type.clone();

    match data_type.to_physical_type() {
        Null => read_null(field_nodes, data_type).map(|x| Arc::new(x) as Arc<dyn Array>),
        Boolean => read_boolean(
            field_nodes,
            data_type,
            buffers,
            reader,
            block_offset,
            is_little_endian,
            compression,
        )
        .map(|x| Arc::new(x) as Arc<dyn Array>),
        Primitive(primitive) => with_match_primitive_type!(primitive, |$T| {
            read_primitive::<$T, _>(
                field_nodes,
                data_type,
                buffers,
                reader,
                block_offset,
                is_little_endian,
                compression,
            )
            .map(|x| Arc::new(x) as Arc<dyn Array>)
        }),
        Binary => {
            let array = read_binary::<i32, _>(
                field_nodes,
                data_type,
                buffers,
                reader,
                block_offset,
                is_little_endian,
                compression,
            )?;
            Ok(Arc::new(array))
        }
        LargeBinary => {
            let array = read_binary::<i64, _>(
                field_nodes,
                data_type,
                buffers,
                reader,
                block_offset,
                is_little_endian,
                compression,
            )?;
            Ok(Arc::new(array))
        }
        FixedSizeBinary => {
            let array = read_fixed_size_binary(
                field_nodes,
                data_type,
                buffers,
                reader,
                block_offset,
                is_little_endian,
                compression,
            )?;
            Ok(Arc::new(array))
        }
        Utf8 => {
            let array = read_utf8::<i32, _>(
                field_nodes,
                data_type,
                buffers,
                reader,
                block_offset,
                is_little_endian,
                compression,
            )?;
            Ok(Arc::new(array))
        }
        LargeUtf8 => {
            let array = read_utf8::<i64, _>(
                field_nodes,
                data_type,
                buffers,
                reader,
                block_offset,
                is_little_endian,
                compression,
            )?;
            Ok(Arc::new(array))
        }
        List => read_list::<i32, _>(
            field_nodes,
            data_type,
            ipc_field,
            buffers,
            reader,
            dictionaries,
            block_offset,
            is_little_endian,
            compression,
            version,
        )
        .map(|x| Arc::new(x) as Arc<dyn Array>),
        LargeList => read_list::<i64, _>(
            field_nodes,
            data_type,
            ipc_field,
            buffers,
            reader,
            dictionaries,
            block_offset,
            is_little_endian,
            compression,
            version,
        )
        .map(|x| Arc::new(x) as Arc<dyn Array>),
        FixedSizeList => read_fixed_size_list(
            field_nodes,
            data_type,
            ipc_field,
            buffers,
            reader,
            dictionaries,
            block_offset,
            is_little_endian,
            compression,
            version,
        )
        .map(|x| Arc::new(x) as Arc<dyn Array>),
        Struct => read_struct(
            field_nodes,
            data_type,
            ipc_field,
            buffers,
            reader,
            dictionaries,
            block_offset,
            is_little_endian,
            compression,
            version,
        )
        .map(|x| Arc::new(x) as Arc<dyn Array>),
        Dictionary(key_type) => {
            match_integer_type!(key_type, |$T| {
                read_dictionary::<$T, _>(
                    field_nodes,
                    ipc_field.dictionary_id,
                    buffers,
                    reader,
                    dictionaries,
                    block_offset,
                    compression,
                    is_little_endian,
                )
                .map(|x| Arc::new(x) as Arc<dyn Array>)
            })
        }
        Union => read_union(
            field_nodes,
            data_type,
            ipc_field,
            buffers,
            reader,
            dictionaries,
            block_offset,
            is_little_endian,
            compression,
            version,
        )
        .map(|x| Arc::new(x) as Arc<dyn Array>),
        Map => read_map(
            field_nodes,
            data_type,
            ipc_field,
            buffers,
            reader,
            dictionaries,
            block_offset,
            is_little_endian,
            compression,
            version,
        )
        .map(|x| Arc::new(x) as Arc<dyn Array>),
    }
}

pub fn skip(
    field_nodes: &mut VecDeque<Node>,
    data_type: &DataType,
    buffers: &mut VecDeque<IpcBuffer>,
) -> Result<()> {
    use PhysicalType::*;
    match data_type.to_physical_type() {
        Null => skip_null(field_nodes),
        Boolean => skip_boolean(field_nodes, buffers),
        Primitive(_) => skip_primitive(field_nodes, buffers),
        LargeBinary | Binary => skip_binary(field_nodes, buffers),
        LargeUtf8 | Utf8 => skip_utf8(field_nodes, buffers),
        FixedSizeBinary => skip_fixed_size_binary(field_nodes, buffers),
        List => skip_list::<i32>(field_nodes, data_type, buffers),
        LargeList => skip_list::<i64>(field_nodes, data_type, buffers),
        FixedSizeList => skip_fixed_size_list(field_nodes, data_type, buffers),
        Struct => skip_struct(field_nodes, data_type, buffers),
        Dictionary(_) => skip_dictionary(field_nodes, buffers),
        Union => skip_union(field_nodes, data_type, buffers),
        Map => skip_map(field_nodes, data_type, buffers),
    }
}
