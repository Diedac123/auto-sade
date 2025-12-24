// Build script para Windows - agrega el icono al ejecutable
fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icono.ico");
        res.compile().expect("Error al compilar recursos de Windows");
    }
}
