fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("bucky-vpn.ico");
        let ret = res.compile();
        if ret.is_err() {
            panic!("compile error: {:?}, {:?}", ret, res);
        }
    }
}
