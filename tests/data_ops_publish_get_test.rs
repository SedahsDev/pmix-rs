use pmix::PmixStatus;
use pmix::data_ops::*;

#[test]
fn test_publish_without_init() {
    let info = pmix::InfoBuilder::new().build();
    let result = publish(&info);
    println!("publish result: {:?}", result);
}

#[test]
fn test_publish_nb_without_init() {
    let info = pmix::InfoBuilder::new().build();
    struct NoOp;
    impl PublishCallback for NoOp {
        fn on_complete(self: Box<Self>, _status: PmixStatus) {}
    }
    let result = publish_nb(&info, Box::new(NoOp));
    println!("publish_nb result: {:?}", result);
}

#[test]
fn test_get_without_init() {
    let proc = pmix::Proc::new("test_ns", 0).unwrap();
    let result = get(&proc, "test_key", None);
    println!("get result: {:?}", result);
}

#[test]
fn test_get_nb_without_init() {
    let proc = pmix::Proc::new("test_ns", 0).unwrap();
    struct NoOp;
    impl GetValueCallback for NoOp {
        fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<pmix::PmixOwnedValue>) {}
    }
    let result = get_nb(&proc, "test_key", None, Box::new(NoOp));
    println!("get_nb result: {:?}", result);
}

#[test]
fn test_get_nb_with_info_without_init() {
    let proc = pmix::Proc::new("test_ns", 0).unwrap();
    let info = pmix::InfoBuilder::new().build();
    struct NoOp;
    impl GetValueCallback for NoOp {
        fn on_result(self: Box<Self>, _status: PmixStatus, _value: Option<pmix::PmixOwnedValue>) {}
    }
    let result = get_nb(&proc, "test_key", Some(&info), Box::new(NoOp));
    println!("get_nb with info result: {:?}", result);
}

#[test]
fn test_get_with_nul_key() {
    let proc = pmix::Proc::new("test_ns", 0).unwrap();
    let result = get(&proc, "test\x00key", None);
    assert!(result.is_err());
}
