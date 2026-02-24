fn main() {
    // Embed the application icon into the Windows executable resource table.
    // This makes the .exe show the icon in Explorer, the taskbar, and Alt-Tab.
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.compile().expect("Failed to compile Windows resource (icon)");
    }
}
