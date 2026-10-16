//! Unter Windows: App-Symbol und Versionsangaben in die mindful.exe einbetten.
//! Auf anderen Systemen tut dieses Skript nichts.

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    // Nur beim Bauen auf Windows für Windows – dort gibt es das nötige Ressourcen-Werkzeug
    if cfg!(windows) && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico")
            .set("ProductName", "Mindful")
            .set(
                "FileDescription",
                "Mindful – mind maps, Markdown and presentations",
            )
            .set("LegalCopyright", "MIT License");
        if let Err(e) = res.compile() {
            println!("cargo:warning=could not embed the Windows icon: {e}");
        }
    }
}
