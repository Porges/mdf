// This code is derived from `snafu`, under the MIT license.

// Copyright (c) 2019- Jake Goulding
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

use std::error::Error;

pub trait AsErrorSource {
    fn as_error_source(&self) -> &(dyn Error + 'static);
}

impl AsErrorSource for dyn Error + 'static {
    fn as_error_source(&self) -> &(dyn Error + 'static) {
        self
    }
}

impl AsErrorSource for dyn Error + Send + 'static {
    fn as_error_source(&self) -> &(dyn Error + 'static) {
        self
    }
}

impl AsErrorSource for dyn Error + Sync + 'static {
    fn as_error_source(&self) -> &(dyn Error + 'static) {
        self
    }
}

impl AsErrorSource for dyn Error + Send + Sync + 'static {
    fn as_error_source(&self) -> &(dyn Error + 'static) {
        self
    }
}

impl<T> AsErrorSource for T
where
    T: Error + 'static,
{
    fn as_error_source(&self) -> &(dyn Error + 'static) {
        self
    }
}
