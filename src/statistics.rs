use crate::*;

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
        if self.times.len() > 0 {
            return self.times.iter().sum::<time::Duration>() / (self.times.len() as u32);
        } else {
            return time::Duration::default();
        }
    }

    pub fn print(&self) {
        println!("{} files in {} (avg {} per file)", self.count(),
            fmt_duration(&self.time_total()), fmt_duration(&self.time_avg()));
    }

    pub fn extend(&mut self, other: &StatisticsItem) {
        self.count += other.count;
        self.times.extend(&other.times);
    }
}

impl Default for StatisticsItem {
    fn default() -> StatisticsItem {
        StatisticsItem { count: 0, times: Vec::new() }
    }
}

impl Statistics {
    pub fn print(&self) {
        print!("Total ");
        self.total.print();
        print!("Decoded ");
        self.decoded.print();
        print!("Encoded ");
        self.encoded.print();
        print!("Copied ");
        self.copied.print();
        print!("Moved ");
        self.moved.print();
        print!("Ignored ");
        self.ignored.print();
        print!("Encountered errors on ");
        self.errors.print();
    }

    pub fn extend(&mut self, other: &Statistics) -> &mut Statistics {
        self.total.extend(&other.total);
        self.decoded.extend(&other.decoded);
        self.encoded.extend(&other.encoded);
        self.copied.extend(&other.copied);
        self.moved.extend(&other.moved);
        self.errors.extend(&other.errors);
        self.ignored.extend(&other.errors);

        return self;
    }
}