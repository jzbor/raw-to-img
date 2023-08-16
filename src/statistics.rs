use crate::*;


#[derive(Default)]
pub struct StatisticsItem {
    count: u32,
    times: Vec<time::Duration>,
}

#[derive(Default)]
pub struct Statistics {
    pub encoded: StatisticsItem,
    pub decoded: StatisticsItem,
    pub copied: StatisticsItem,
    pub moved: StatisticsItem,
    pub ignored: StatisticsItem,
    pub errors: StatisticsItem,
    pub total: StatisticsItem,
}


impl StatisticsItem {
    pub fn record(&mut self, time: time::Duration) {
        self.times.push(time);
        self.count += 1;
    }

    pub fn inc(&mut self) {
        self.count += 1;
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    pub fn time_total(&self) -> time::Duration {
        self.times.iter().sum()
    }

    pub fn time_avg(&self) -> time::Duration {
        if !self.times.is_empty() {
            return self.times.iter().sum::<time::Duration>() / (self.times.len() as u32);
        } else {
            time::Duration::default()
        }
    }

    pub fn print(&self) {
        println!("{} files in {} (avg {} per file)", self.count(),
            fmt_duration(&self.time_total()), fmt_duration(&self.time_avg()));
    }

    pub fn print_nthreads(&self, nthreads: u32) {
        println!("{} files in approx. {} (avg {} per file)", self.count(),
            fmt_duration(&(self.time_total() / nthreads)), fmt_duration(&self.time_avg()));
    }

    pub fn extend(&mut self, other: &StatisticsItem) {
        self.count += other.count;
        self.times.extend(&other.times);
    }
}

impl Statistics {
    pub fn print_nthreads(&self, nthreads: u32) {
        print!("Total ");
        self.total.print();
        print!("Decoded ");
        self.decoded.print_nthreads(nthreads);
        print!("Encoded ");
        self.encoded.print_nthreads(nthreads);
        print!("Copied ");
        self.copied.print_nthreads(nthreads);
        print!("Moved ");
        self.moved.print_nthreads(nthreads);
        print!("Ignored ");
        self.ignored.print_nthreads(nthreads);
        print!("Encountered errors on ");
        self.errors.print_nthreads(nthreads);
    }

    pub fn extend(&mut self, other: &Statistics) -> &mut Statistics {
        self.total.extend(&other.total);
        self.decoded.extend(&other.decoded);
        self.encoded.extend(&other.encoded);
        self.copied.extend(&other.copied);
        self.moved.extend(&other.moved);
        self.errors.extend(&other.errors);
        self.ignored.extend(&other.ignored);

        self
    }
}
