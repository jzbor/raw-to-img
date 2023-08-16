use crate::*;

pub struct Job {
    input_file: PathBuf,
    output_file: PathBuf,
    on_raw: ParsableAction,
    on_file: UnparsableAction,
    on_image: UnparsableAction,
    on_existing: ExistingAction,
    encoder: EncoderType,
    statistics: Statistics,
}


impl Job {
    pub fn new(input_file: &Path, output_file: &Path, on_raw: ParsableAction,
           on_file: UnparsableAction, on_image: UnparsableAction, on_existing: ExistingAction,
           encoder: EncoderType) -> Job {
        Job {
            input_file: input_file.to_path_buf(),
            output_file: output_file.to_path_buf(),
            on_raw, on_file, on_image, on_existing, encoder,
            statistics: Statistics::default(),
        }
    }

    pub fn name(&self) -> String {
        return self.input_file.to_string_lossy().to_string();
    }

    pub fn run(mut self) -> Result<Statistics, String> {
        // fetch file metadata to later distinguish regular files from other files
        let metadata = self.input_file.metadata()
            .map_err(|s| s.to_string())?;

        // create parent directory if necessary
        if self.output_file.parent().is_some() && !self.output_file.parent().unwrap().exists() {
            fs::create_dir_all((self.output_file.parent()).unwrap()).map_err(|s| s.to_string())?;
        }

        if metadata.is_file() {
            if self.output_file.exists() {
                match self.on_existing {
                    ExistingAction::Rename => {
                        self.statistics.errors.inc();
                        return Err(format!("Could not find unused path for {}", self.output_file.to_string_lossy()));
                    },
                    ExistingAction::Ignore => {
                        self.statistics.ignored.inc();
                        return Ok(self.statistics);
                    }
                }
            }

            match file_kind(&self.input_file) {
                FileKind::Raw => match self.on_raw {
                    ParsableAction::Ignore => self.statistics.ignored.inc(),
                    ParsableAction::Parse =>
                        match recode(self.input_file.as_path(), self.output_file.as_path(), self.encoder) {
                            Some((dtime, etime)) => {
                                self.statistics.decoded.record(dtime);
                                self.statistics.encoded.record(etime);
                            },
                            None => self.statistics.errors.inc(),
                        },
                    ParsableAction::Copy =>
                        match copy(self.input_file.as_path(), self.output_file.as_path()) {
                            Some(ctime) => self.statistics.copied.record(ctime),
                            None => self.statistics.errors.inc(),
                        },
                    ParsableAction::Move =>
                        match move_file(self.input_file.as_path(), self.output_file.as_path()) {
                            Some(mtime) => self.statistics.moved.record(mtime),
                            None => self.statistics.errors.inc(),
                        },
                },
                FileKind::Image => match self.on_image {
                    UnparsableAction::Ignore => self.statistics.ignored.inc(),
                    UnparsableAction::Copy =>
                        match copy(self.input_file.as_path(), self.output_file.as_path()) {
                            Some(ctime) => self.statistics.copied.record(ctime),
                            None => self.statistics.errors.inc(),
                        },
                    UnparsableAction::Move =>
                        match move_file(self.input_file.as_path(), self.output_file.as_path()) {
                            Some(mtime) => self.statistics.moved.record(mtime),
                            None => self.statistics.errors.inc(),
                        },
                },
                FileKind::Other => match self.on_file {
                    UnparsableAction::Ignore => self.statistics.ignored.inc(),
                    UnparsableAction::Copy =>
                        match copy(self.input_file.as_path(), self.output_file.as_path()) {
                            Some(ctime) => self.statistics.copied.record(ctime),
                            None => self.statistics.errors.inc(),
                        },
                    UnparsableAction::Move =>
                        match move_file(self.input_file.as_path(), self.output_file.as_path()) {
                            Some(mtime) => self.statistics.moved.record(mtime),
                            None => self.statistics.errors.inc(),
                        },
                },
            }
        } else {
            self.statistics.ignored.inc();
        }

        Ok(self.statistics)
    }
}


