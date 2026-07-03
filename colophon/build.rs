use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=data/org.virinvictus.Colophon.gschema.xml");
    // Compile the GSettings schema for dev runs (main.rs points
    // GSETTINGS_SCHEMA_DIR at data/). Non-fatal: without it the app falls
    // back to defaults via the settings() Option accessor.
    match Command::new("glib-compile-schemas").arg("data/").status() {
        Ok(status) if status.success() => {}
        Ok(status) => println!("cargo:warning=glib-compile-schemas exited with {status}"),
        Err(err) => println!("cargo:warning=glib-compile-schemas not run: {err}"),
    }
}
