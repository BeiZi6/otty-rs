fn main() {
    // The Slint compiler runs at build time, so a layout error is a build error.
    slint_build::compile("ui/app.slint").expect("the Slint layout must compile");
}

