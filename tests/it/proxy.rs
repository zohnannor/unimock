use unimock::*;

mod upstream {
    pub trait Upstream {
        fn foo(&self, arg: &str) -> i32;
    }
}

struct Proxy(Unimock);

#[unimock(proxy = upstream::Upstream for Proxy, api = UpstreamMock)]
trait Whatever {
    fn foo(&self, arg: &str) -> i32;
}

#[test]
fn it_works() {
    let u = Proxy(Unimock::new(
        UpstreamMock::foo.next_call(matching!("a")).returns(42),
    ));

    use upstream::Upstream;
    assert_eq!(42, u.foo("a"));
}
