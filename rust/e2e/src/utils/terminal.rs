use indicatif::ProgressStyle;

pub fn spinner(prefix: &str, msg: &str) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new_spinner();
    pb.enable_steady_tick(120);
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&[
                "▹▹▹▹▹",
                "▸▹▹▹▹",
                "▹▸▹▹▹",
                "▹▹▸▹▹",
                "▹▹▹▸▹",
                "▹▹▹▹▸",
                "▪▪▪▪▪",
            ])
            .template("{prefix:<25.bold.dim} {spinner:.blue} [{elapsed_precise}] {msg}"),
    );
    pb.set_message(msg);
    pb.set_prefix(prefix);
    pb
}
