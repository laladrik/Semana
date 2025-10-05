fn main() {
    println!("cargo::rustc-link-arg=-Wl,-rpath,./../dependencies/SDL_ttf/build/build-linux");
    println!("cargo::rustc-link-arg=-Wl,-rpath,./../dependencies/SDL-release-3.2.12/build");
}
