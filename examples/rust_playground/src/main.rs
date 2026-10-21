use arctos_rust_playground::parabola_samples;

fn main() {
    for (x, y) in parabola_samples(9) {
        println!("{x:>4} | {y}");
    }
}
