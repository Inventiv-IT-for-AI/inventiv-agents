fn main() {
    // sqlx::migrate! embeds migrations at compile-time.
    // Ensure Cargo rebuilds this crate whenever migrations change.
    println!("cargo:rerun-if-changed=../sqlx-migrations");
}

