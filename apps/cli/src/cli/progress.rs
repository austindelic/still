use indicatif::ProgressBar;
#[cfg(not(test))]
use indicatif::ProgressStyle;

#[derive(Debug)]
pub struct Spinner {
    bar: Option<ProgressBar>,
}

impl Spinner {
    pub fn start(message: impl Into<String>) -> Self {
        #[cfg(test)]
        {
            let _ = message.into();
            Self { bar: None }
        }

        #[cfg(not(test))]
        {
            let bar = ProgressBar::new_spinner();
            bar.set_style(
                ProgressStyle::with_template("{spinner:.cyan} {msg}")
                    .unwrap_or_else(|_| ProgressStyle::default_spinner()),
            );
            bar.enable_steady_tick(std::time::Duration::from_millis(100));
            bar.set_message(message.into());
            Self { bar: Some(bar) }
        }
    }

    pub fn finish(self) {
        if let Some(bar) = self.bar {
            bar.finish_and_clear();
        }
    }
}
