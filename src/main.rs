use vortexlab::cases::poiseuille;
use vortexlab::lattice::Collision;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("poiseuille") => {
            for (name, col, tau, q) in [
                ("bgk tau=0.8 halfway", Collision::Bgk, 0.8, 0.5),
                ("trt 3/16 halfway", Collision::Trt { magic: 0.1875 }, 0.8, 0.5),
                ("trt 1/4 halfway", Collision::Trt { magic: 0.25 }, 0.8, 0.5),
                ("trt 3/16 q=0.25", Collision::Trt { magic: 0.1875 }, 0.8, 0.25),
                ("trt 3/16 q=0.8", Collision::Trt { magic: 0.1875 }, 0.8, 0.8),
            ] {
                let out: Vec<_> = [8, 16, 32, 64].iter().map(|&h| poiseuille::run(h, tau, col, q, 1)).collect();
                let e: Vec<String> = out.iter().map(|o| format!("{:.3e}", o.l2)).collect();
                let fb: Vec<String> = out.iter().map(|o| format!("{:.12}", o.force_balance)).collect();
                println!("{name}: l2 {e:?} orders {:?} balance {fb:?} steps {}", poiseuille::observed_orders(&out), out[3].steps);
            }
        }
        _ => eprintln!("usage: vortexlab <command>"),
    }
}
