use unimock_macros::unimock;

pub mod io {
    use super::*;
    use std::fmt;
    use std::io::{Error, IoSlice, IoSliceMut};

    /*
    #[unimock(prefix = crate, api = ReadMock, emulate = std::io::Read)]
    pub trait Read {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error>;

        fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize, Error>;

        fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize, Error>;

        fn read_to_string(&mut self, buf: &mut String) -> Result<usize, Error>;

        fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error>;

        // unstable methods:
        // fn is_read_vectored(&self) -> bool;
        // fn read_buf(&mut self, buf: BorrowedCursor<'_>) -> Result<(), Error>;
        // fn read_buf_exact(&mut self, mut cursor: BorrowedCursor<'_>) -> Result<(), Error>;
    }
    */

    #[doc = "Unimock setup module for `Read`"]
    #[allow(non_snake_case)]
    pub mod ReadMock {
        #[allow(non_camel_case_types)]
        #[doc = "MockFn for `Read::read(buf: &mut [u8]) -> Result<usize, Error>`."]
        pub struct read;

        #[allow(non_camel_case_types)]
        #[doc = "MockFn for `Read::read_vectored(bufs: &mut [IoSliceMut<'_>]) -> Result<usize, Error>`."]
        pub struct read_vectored;

        #[allow(non_camel_case_types)]
        #[doc = "MockFn for `Read::read_to_end(buf: &mut Vec<u8>) -> Result<usize, Error>`."]
        pub struct read_to_end;

        #[allow(non_camel_case_types)]
        #[doc = "MockFn for `Read::read_to_string(buf: &mut String) -> Result<usize, Error>`."]
        pub struct read_to_string;

        #[allow(non_camel_case_types)]
        #[doc = "MockFn for `Read::read_exact(buf: &mut [u8]) -> Result<(), Error>`."]
        pub struct read_exact;
    }
    const _: () = {
        impl crate::MockFn for ReadMock::read {
            type Inputs<'__i> = (&'__i mut [u8]);
            type Response = crate::output::Owned<Result<usize, Error>>;
            type Output<'u> = crate::output::Owned<Result<usize, Error>>;
            const NAME: &'static str = "Read::read";
            fn debug_inputs((buf): &Self::Inputs<'_>) -> String {
                use crate::macro_api::{NoDebug, ProperDebug};
                crate::macro_api::format_inputs(&[(*buf).unimock_try_debug()])
            }
        }
        impl crate::MockFn for ReadMock::read_vectored {
            type Inputs<'__i> = (&'__i mut [IoSliceMut<'__i>]);
            type Response = crate::output::Owned<Result<usize, Error>>;
            type Output<'u> = crate::output::Owned<Result<usize, Error>>;
            const NAME: &'static str = "Read::read_vectored";
            fn debug_inputs((bufs): &Self::Inputs<'_>) -> String {
                use crate::macro_api::{NoDebug, ProperDebug};
                crate::macro_api::format_inputs(&[(*bufs).unimock_try_debug()])
            }
        }
        impl crate::MockFn for ReadMock::read_to_end {
            type Inputs<'__i> = (&'__i mut Vec<u8>);
            type Response = crate::output::Owned<Result<usize, Error>>;
            type Output<'u> = crate::output::Owned<Result<usize, Error>>;
            const NAME: &'static str = "Read::read_to_end";
            fn debug_inputs((buf): &Self::Inputs<'_>) -> String {
                use crate::macro_api::{NoDebug, ProperDebug};
                crate::macro_api::format_inputs(&[(*buf).unimock_try_debug()])
            }
        }
        impl crate::MockFn for ReadMock::read_to_string {
            type Inputs<'__i> = (&'__i mut String);
            type Response = crate::output::Owned<Result<usize, Error>>;
            type Output<'u> = crate::output::Owned<Result<usize, Error>>;
            const NAME: &'static str = "Read::read_to_string";
            fn debug_inputs((buf): &Self::Inputs<'_>) -> String {
                use crate::macro_api::{NoDebug, ProperDebug};
                crate::macro_api::format_inputs(&[(*buf).unimock_try_debug()])
            }
        }
        impl crate::MockFn for ReadMock::read_exact {
            type Inputs<'__i> = (&'__i mut [u8]);
            type Response = crate::output::Owned<Result<(), Error>>;
            type Output<'u> = crate::output::Owned<Result<(), Error>>;
            const NAME: &'static str = "Read::read_exact";
            fn debug_inputs((buf): &Self::Inputs<'_>) -> String {
                use crate::macro_api::{NoDebug, ProperDebug};
                crate::macro_api::format_inputs(&[(*buf).unimock_try_debug()])
            }
        }
        impl std::io::Read for crate::Unimock {
            #[track_caller]
            fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
                crate::macro_api::eval::<ReadMock::read>(&self, (buf)).unwrap(&self)
            }
            #[track_caller]
            fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize, Error> {
                crate::macro_api::eval::<ReadMock::read_vectored>(&self, (bufs)).unwrap(&self)
            }
            #[track_caller]
            fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize, Error> {
                crate::macro_api::eval::<ReadMock::read_to_end>(&self, (buf)).unwrap(&self)
            }
            #[track_caller]
            fn read_to_string(&mut self, buf: &mut String) -> Result<usize, Error> {
                crate::macro_api::eval::<ReadMock::read_to_string>(&self, (buf)).unwrap(&self)
            }
            #[track_caller]
            fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Error> {
                crate::macro_api::eval::<ReadMock::read_exact>(&self, (buf)).unwrap(&self)
            }
        }
    };

    #[unimock(prefix = crate, api = WriteMock, emulate = std::io::Write)]
    pub trait Write {
        fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error>;

        fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize, std::io::Error>;

        fn flush(&mut self) -> Result<(), std::io::Error>;

        fn write_all(&mut self, buf: &[u8]) -> Result<(), std::io::Error>;

        fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> Result<(), std::io::Error>;
    }
}
