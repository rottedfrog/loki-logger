use std::io::Result;
fn main() -> Result<()> {
    prost_build::compile_protos(
        &["src/logproto/push.proto"],
        &["src/logproto/", "loki/vendor/github.com/"],
    )?;
    println!("{}", std::env::var("OUT_DIR").unwrap());
    Ok(())
}
