use std::fmt::Display;
use std::sync::Arc;

use arrow_array::{make_array, Array, ArrayRef};
use arrow_schema::{Field, FieldRef};
use pyo3::intern;
use pyo3::prelude::*;
use pyo3::types::{PyCapsule, PyTuple, PyType};

use crate::error::PyArrowResult;
use crate::ffi::from_python::utils::import_array_pycapsules;
use crate::ffi::to_array_pycapsules;
use crate::ffi::to_python::nanoarrow::to_nanoarrow_array;
use crate::interop::numpy::to_numpy::to_numpy;

/// A Python-facing Arrow array.
///
/// This is a wrapper around an [ArrayRef] and a [FieldRef].
#[pyclass(module = "arro3.core._rust", name = "Array", subclass)]
pub struct PyArray {
    array: ArrayRef,
    field: FieldRef,
}

impl PyArray {
    /// Create a new Python Array from an [ArrayRef] and a [FieldRef].
    pub fn new(array: ArrayRef, field: FieldRef) -> Self {
        Self { array, field }
    }

    pub fn from_array<A: Array>(array: A) -> Self {
        let array = make_array(array.into_data());
        Self::from_array_ref(array)
    }

    /// Create a new PyArray from an [ArrayRef], inferring its data type automatically.
    pub fn from_array_ref(array: ArrayRef) -> Self {
        let field = Field::new("", array.data_type().clone(), true);
        Self::new(array, Arc::new(field))
    }

    /// Access the underlying [ArrayRef].
    pub fn array(&self) -> &ArrayRef {
        &self.array
    }

    /// Access the underlying [FieldRef].
    pub fn field(&self) -> &FieldRef {
        &self.field
    }

    /// Consume self to access the underlying [ArrayRef] and [FieldRef].
    pub fn into_inner(self) -> (ArrayRef, FieldRef) {
        (self.array, self.field)
    }

    /// Export to an arro3.core.Array.
    ///
    /// This requires that you depend on arro3-core from your Python package.
    pub fn to_arro3(&self, py: Python) -> PyResult<PyObject> {
        let arro3_mod = py.import_bound(intern!(py, "arro3.core"))?;
        let core_obj = arro3_mod.getattr(intern!(py, "Array"))?.call_method1(
            intern!(py, "from_arrow_pycapsule"),
            self.__arrow_c_array__(py, None)?,
        )?;
        Ok(core_obj.to_object(py))
    }

    /// Export this to a Python `nanoarrow.Array`.
    pub fn to_nanoarrow(&self, py: Python) -> PyResult<PyObject> {
        to_nanoarrow_array(py, &self.__arrow_c_array__(py, None)?)
    }

    /// Export to a pyarrow.Array
    ///
    /// Requires pyarrow >=14
    pub fn to_pyarrow(self, py: Python) -> PyResult<PyObject> {
        let pyarrow_mod = py.import_bound(intern!(py, "pyarrow"))?;
        let pyarrow_obj = pyarrow_mod
            .getattr(intern!(py, "array"))?
            .call1(PyTuple::new_bound(py, vec![self.into_py(py)]))?;
        Ok(pyarrow_obj.to_object(py))
    }
}

impl From<ArrayRef> for PyArray {
    fn from(value: ArrayRef) -> Self {
        Self::from_array_ref(value)
    }
}

impl AsRef<ArrayRef> for PyArray {
    fn as_ref(&self) -> &ArrayRef {
        &self.array
    }
}

impl Display for PyArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "arro3.core.Array<")?;
        self.array.data_type().fmt(f)?;
        writeln!(f, ">")?;
        Ok(())
    }
}

#[pymethods]
impl PyArray {
    /// An implementation of the Array interface, for interoperability with numpy and other
    /// array libraries.
    pub fn __array__(&self, py: Python) -> PyResult<PyObject> {
        to_numpy(py, &self.array)
    }

    /// An implementation of the [Arrow PyCapsule
    /// Interface](https://arrow.apache.org/docs/format/CDataInterface/PyCapsuleInterface.html).
    /// This dunder method should not be called directly, but enables zero-copy
    /// data transfer to other Python libraries that understand Arrow memory.
    ///
    /// For example, you can call [`pyarrow.array()`][pyarrow.array] to convert this array
    /// into a pyarrow array, without copying memory.
    #[allow(unused_variables)]
    pub fn __arrow_c_array__<'py>(
        &'py self,
        py: Python<'py>,
        requested_schema: Option<Bound<PyCapsule>>,
    ) -> PyArrowResult<Bound<PyTuple>> {
        to_array_pycapsules(py, self.field.clone(), &self.array, requested_schema)
    }

    pub fn __eq__(&self, other: &PyArray) -> bool {
        self.array.as_ref() == other.array.as_ref() && self.field == other.field
    }

    pub fn __len__(&self) -> usize {
        self.array.len()
    }

    pub fn __repr__(&self) -> String {
        self.to_string()
    }

    /// Construct this object from an existing Arrow object.
    ///
    /// It can be called on anything that exports the Arrow data interface
    /// (`__arrow_c_array__`).
    ///
    /// Args:
    ///     input: Arrow array to use for constructing this object
    ///
    /// Returns:
    ///     Self
    #[classmethod]
    pub fn from_arrow(_cls: &Bound<PyType>, input: &Bound<PyAny>) -> PyResult<Self> {
        input.extract()
    }

    /// Construct this object from a bare Arrow PyCapsule
    #[classmethod]
    pub fn from_arrow_pycapsule(
        _cls: &Bound<PyType>,
        schema_capsule: &Bound<PyCapsule>,
        array_capsule: &Bound<PyCapsule>,
    ) -> PyResult<Self> {
        let (array, field) = import_array_pycapsules(schema_capsule, array_capsule)?;
        Ok(Self::new(array, Arc::new(field)))
    }

    /// Copy this array to a `numpy` NDArray
    pub fn to_numpy(&self, py: Python) -> PyResult<PyObject> {
        self.__array__(py)
    }
}
