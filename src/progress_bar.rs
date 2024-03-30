use std::fmt::Write;

pub fn eta_key(state: &indicatif::ProgressState, f: &mut dyn Write) {
    write!(f, "{:.1}s", state.eta().as_secs_f64()).unwrap()
}

#[macro_export]
macro_rules! init_progress {
    ($local:expr, $label:expr) => {{
        let pb = indicatif::ProgressBar::new($local as u64);
        let mut template =
            "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ".to_string();
        template += $label;
        template += " ({eta})";
        pb.set_style(
            indicatif::ProgressStyle::with_template(&template)
                .unwrap()
                .with_key("eta", $crate::progress_bar::eta_key)
                .progress_chars("#>-"),
        );
        pb
    }};
}

#[macro_export]
macro_rules! update_progress {
    ($pb:ident, $index:expr) => {
        $pb.set_position(($index + 1) as u64);
    };
}

#[macro_export]
macro_rules! update_progress_by_one {
    ($pb:ident) => {
        $pb.inc(1);
    };
}

#[macro_export]
macro_rules! finish_progress {
    ($pb:ident) => {
        $pb.finish();
    };
}
