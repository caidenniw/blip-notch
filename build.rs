fn main() {
    embed_resource::compile("assets/blip.rc", embed_resource::NONE)
        .manifest_required()
        .unwrap();
}
