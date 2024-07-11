extern crate embed_resource;
fn main() {
    println!("cargo:rerun-if-changed=resource.rc");
    embed_resource::compile("resource.rc", embed_resource::NONE);
}