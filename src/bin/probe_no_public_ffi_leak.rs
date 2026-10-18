use pmix::data_serialization::{PmixDataBuffer, data_buffer_create};

fn main() {
    let _buf = data_buffer_create().expect("create buffer");
}
