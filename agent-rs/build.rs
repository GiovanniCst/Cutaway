// Puts the icon and the version fields inside the executable.
//
// The agent already carries the .ico bytes and builds its notification-area
// icon from them at run time, which is the one people see. This is the other
// one: the file's own icon, in Explorer and in Task Manager's startup list,
// where a background program that starts at logon had better look like
// something recognisable.

fn main() {
    println!("cargo:rerun-if-changed=../assets/cutaway.ico");
    if !cfg!(target_os = "windows") {
        return;
    }
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("../assets/cutaway.ico");
    resource.set("ProductName", "Cutaway");
    resource.set("FileDescription", "Cutaway - the shortcut that cuts a piece of the screen");
    resource.set("CompanyName", "Giovanni J. Costantini");
    resource.set("LegalCopyright", "Apache-2.0");
    resource.set("OriginalFilename", "CutawayAgent.exe");
    // Common controls version 6, which is where TaskDialogIndirect lives: the
    // About dialog does not open without it. No dpiAware element in there - the
    // agent declares its own awareness in code, before it reads a screen.
    resource.set_manifest_file("cutaway-agent.manifest");
    if let Err(trouble) = resource.compile() {
        println!("cargo:warning=nessuna risorsa Windows: {trouble}");
    }
}
