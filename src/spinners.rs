use indicatif::{ProgressBar, ProgressDrawTarget, ProgressFinish, ProgressStyle};

pub fn progress_spinner() -> anyhow::Result<ProgressBar> {
    Ok(
        ProgressBar::with_draw_target(None, ProgressDrawTarget::stderr()).with_style(
            ProgressStyle::with_template("{spinner:.green} {pos} - {msg}")?,
        ),
    )
}

pub fn progress_bar(size: usize) -> ProgressBar {
    ProgressBar::new(size as u64)
        .with_style(
            ProgressStyle::with_template(
                "{spinner:.green} {msg} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})",
            )
                .expect("template is correct")
                .progress_chars("#>-"),
        )
        .with_finish(ProgressFinish::AndLeave)
}
