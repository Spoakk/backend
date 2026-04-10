fn main() {
    cc::Build::new()
        .files([
            "cubiomes/generator.c",
            "cubiomes/biomenoise.c",
            "cubiomes/biomes.c",
            "cubiomes/layers.c",
            "cubiomes/noise.c",
            "cubiomes/util.c",
            "cubiomes/finders.c",
            "cubiomes/quadbase.c",
            "cubiomes_shim.c",
        ])
        .include("cubiomes")
        .flag_if_supported("-fwrapv")
        .flag_if_supported("-O3")
        .compile("cubiomes");

    println!("cargo:rerun-if-changed=cubiomes_shim.c");
    println!("cargo:rerun-if-changed=cubiomes/generator.c");
    println!("cargo:rerun-if-changed=cubiomes/generator.h");
}
