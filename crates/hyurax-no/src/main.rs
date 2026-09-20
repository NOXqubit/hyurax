// O programa de terminal. Toda a lógica mora na biblioteca (src/lib.rs), que o
// programa com janela (src/bin/hyurax.rs) também usa.

fn main() -> std::process::ExitCode {
    hyurax_no::executar()
}
