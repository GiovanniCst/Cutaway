// Puts the icon and the version fields inside the executable.
//
// The window has carried its icon since the beginning, but that is the running
// program: Explorer, the desktop shortcut and a pinned taskbar button read the
// file, and the file had nothing to read. A program whose shortcut is the blank
// default page looks like something that failed to install.
//
// The same resource carries the version, which is what the Properties dialog
// and Add/Remove Programs show. It comes from Cargo.toml rather than from a
// second place that could disagree with it.

fn main() {
    println!("cargo:rerun-if-changed=../assets/cutaway.ico");
    if !cfg!(target_os = "windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("../assets/cutaway.ico");
    resource.set("ProductName", "Cutaway");
    resource.set("FileDescription", "Cutaway");
    resource.set("CompanyName", "Giovanni J. Costantini");
    resource.set("LegalCopyright", "Apache-2.0");
    resource.set("OriginalFilename", "Cutaway.exe");
    // Not fatal. A machine without the Windows SDK can still build and run a
    // program that works; what it loses is the icon on the file, and stopping
    // the build over that would be the wrong trade.
    if let Err(trouble) = resource.compile() {
        println!("cargo:warning=nessuna risorsa Windows: {trouble}");
    }
}
