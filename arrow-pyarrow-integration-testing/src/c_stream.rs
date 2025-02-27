//! This library demonstrates a minimal usage of Rust's C data interface to pass
//! arrays from and to Python.
use pyo3::ffi::Py_uintptr_t;
use pyo3::prelude::*;

use arrow2::array::{Int32Array, StructArray};
use arrow2::datatypes::DataType;
use arrow2::ffi;

use super::*;

pub fn to_rust_iterator(ob: PyObject, py: Python) -> PyResult<Vec<PyObject>> {
    let stream = Box::new(ffi::ArrowArrayStream::empty());

    let stream_ptr = &*stream as *const ffi::ArrowArrayStream;

    // make the conversion through PyArrow's private API
    // this changes the pointer's memory and is thus unsafe. In particular, `_export_to_c` can go out of bounds
    ob.call_method1(py, "_export_to_c", (stream_ptr as Py_uintptr_t,))?;

    let mut iter =
        unsafe { ffi::ArrowArrayStreamReader::try_new(stream).map_err(PyO3ArrowError::from) }?;

    let mut arrays = vec![];
    while let Some(array) = unsafe { iter.next() } {
        let py_array = to_py_array(array.unwrap().into(), py)?;
        arrays.push(py_array)
    }
    Ok(arrays)
}

pub fn from_rust_iterator(py: Python) -> PyResult<PyObject> {
    // initialize an array
    let array = Int32Array::from(&[Some(2), None, Some(1), None]);
    let array = StructArray::from_data(
        DataType::Struct(vec![Field::new("a", array.data_type().clone(), true)]),
        vec![Arc::new(array)],
        None,
    );
    // and a field with its datatype
    let field = Field::new("a", array.data_type().clone(), true);

    // Arc it, since it will be shared with an external program
    let array: Arc<dyn Array> = Arc::new(array.clone());

    // create an iterator of arrays
    let arrays = vec![array.clone(), array.clone(), array];
    let iter = Box::new(arrays.clone().into_iter().map(Ok)) as _;

    // create an [`ArrowArrayStream`] based on this iterator and field
    let mut stream = Box::new(ffi::ArrowArrayStream::empty());
    unsafe { ffi::export_iterator(iter, field, &mut *stream) };

    // call pyarrow's interface to read this stream
    let pa = py.import("pyarrow.ipc")?;
    let py_stream = pa.getattr("RecordBatchReader")?.call_method1(
        "_import_from_c",
        ((&*stream as *const ffi::ArrowArrayStream) as Py_uintptr_t,),
    )?;
    Box::leak(stream); // this is not ideal => the struct should be allocated by pyarrow

    Ok(py_stream.to_object(py))
}
