use chrono::{DateTime, Utc};
use std::collections::BTreeMap;

use crate::{
    data_combination::data_points::data_point::{
        CalcWasActiveEachPastPeriod,
        CalculateDataPoints,
        DataPointMetadata,
    },
    database::analytics_event::data_model::AnalyticsEvent,
};

impl<'a> CalcWasActiveEachPastPeriod<'a> {
    pub fn new(
        metadata: DataPointMetadata,
        events: Vec<AnalyticsEvent>,
        period_thresholds: Vec<DateTime<Utc>>,
    ) -> CalcWasActiveEachPastPeriod {
        CalcWasActiveEachPastPeriod {
            metadata,
            events,
            period_thresholds,
        }
    }

    // TODO: this could possibly later be optimised by sorting events by timestamp and caching the last timestamp added to the HashMap
    fn group_timestamps_by_period_threshold(&self) -> BTreeMap<DateTime<Utc>, Vec<DateTime<Utc>>> {
        let mut timestamps_by_period_threshold = BTreeMap::new();
        for these_thresholds in self.period_thresholds.windows(2) {
            // safe unwrap: `windows` guarantees that there are at least two elements.
            // If `period_thresholds` contains less than two elements, this code block is not executed
            let newer_threshold = these_thresholds.first().unwrap();
            let older_threshold = these_thresholds.last().unwrap();
            let timestamps: Vec<DateTime<Utc>> = self
                .events
                .iter()
                .filter(|event| {
                    event.timestamp < *newer_threshold && event.timestamp > *older_threshold
                })
                .map(|event| event.timestamp)
                .collect();
            timestamps_by_period_threshold.insert(*newer_threshold, timestamps);
        }
        timestamps_by_period_threshold
    }
}

impl<'a> CalculateDataPoints for CalcWasActiveEachPastPeriod<'a> {
    fn metadata(&self) -> DataPointMetadata {
        self.metadata
    }

    fn calculate(&self) -> Vec<u32> {
        let timestamps_by_period_threshold = self.group_timestamps_by_period_threshold();
        // since we are travelling 'back in time' we need to reverse the order of the values of the BTreeMap
        timestamps_by_period_threshold
            .values()
            .rev()
            .map(|timestamps| !timestamps.is_empty() as u32)
            .collect::<Vec<u32>>()
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration, Utc};

    use super::*;
    use crate::{
        data_combination::data_points::data_point::{Period, PeriodUnit},
        database::analytics_event::data_model::AnalyticsEventType,
    };

    #[test]
    fn test_calculate_no_events_in_a_period() {
        let end_period = DateTime::parse_from_rfc3339("2021-02-02T00:00:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 1), end_period);
        let period_thresholds = vec![end_period, end_period - Duration::days(1)];
        let was_active_each_past_period =
            CalcWasActiveEachPastPeriod::new(metadata, Vec::new(), period_thresholds);
        assert_eq!(was_active_each_past_period.calculate(), vec![0]);
    }

    #[test]
    fn test_calculate_one_event_in_a_period() {
        let end_period = DateTime::parse_from_rfc3339("2021-03-03T00:00:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 1), end_period);
        let events = vec![AnalyticsEvent::new(
            "test1",
            AnalyticsEventType::UserAction,
            end_period - Duration::hours(12),
            None,
        )];
        let period_thresholds = vec![end_period, end_period - Duration::days(1)];
        let was_active_each_past_period =
            CalcWasActiveEachPastPeriod::new(metadata, events, period_thresholds);
        assert_eq!(was_active_each_past_period.calculate(), vec![1]);
    }

    #[test]
    fn test_calculate_no_events_in_two_periods() {
        let end_period = DateTime::parse_from_rfc3339("2021-04-04T00:00:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 2), end_period);
        let period_thresholds = vec![
            end_period,
            end_period - Duration::days(1),
            end_period - Duration::days(2),
        ];
        let was_active_each_past_period =
            CalcWasActiveEachPastPeriod::new(metadata, Vec::new(), period_thresholds);
        assert_eq!(was_active_each_past_period.calculate(), vec![0, 0]);
    }

    #[test]
    fn test_calculate_one_event_in_one_period_zero_in_another() {
        let end_period = DateTime::parse_from_rfc3339("2021-05-05T00:00:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 2), end_period);
        let events = vec![AnalyticsEvent::new(
            "test1",
            AnalyticsEventType::UserAction,
            end_period - Duration::hours(12),
            None,
        )];
        let period_thresholds = vec![
            end_period,
            end_period - Duration::days(1),
            end_period - Duration::days(2),
        ];
        let was_active_each_past_period =
            CalcWasActiveEachPastPeriod::new(metadata, events, period_thresholds);
        assert_eq!(was_active_each_past_period.calculate(), vec![1, 0]);
    }

    #[test]
    fn test_calculate_two_events_in_one_period_zero_in_another() {
        let end_period = DateTime::parse_from_rfc3339("2021-06-06T00:00:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 2), end_period);
        let events = vec![
            AnalyticsEvent::new(
                "test1",
                AnalyticsEventType::UserAction,
                end_period - Duration::hours(12),
                None,
            ),
            AnalyticsEvent::new(
                "test2",
                AnalyticsEventType::Error,
                end_period - Duration::hours(15),
                None,
            ),
        ];
        let period_thresholds = vec![
            end_period,
            end_period - Duration::days(1),
            end_period - Duration::days(2),
        ];
        let was_active_each_past_period =
            CalcWasActiveEachPastPeriod::new(metadata, events, period_thresholds);
        assert_eq!(was_active_each_past_period.calculate(), vec![1, 0]);
    }

    #[test]
    fn test_calculate_two_periods_with_one_event_each() {
        let end_period = DateTime::parse_from_rfc3339("2021-07-07T00:00:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 2), end_period);
        let events = vec![
            AnalyticsEvent::new(
                "test1",
                AnalyticsEventType::UserAction,
                end_period - Duration::hours(12),
                None,
            ),
            AnalyticsEvent::new(
                "test2",
                AnalyticsEventType::Error,
                end_period - Duration::hours(36),
                None,
            ),
        ];
        let period_thresholds = vec![
            end_period,
            end_period - Duration::days(1),
            end_period - Duration::days(2),
        ];
        let was_active_each_past_period =
            CalcWasActiveEachPastPeriod::new(metadata, events, period_thresholds);
        assert_eq!(was_active_each_past_period.calculate(), vec![1, 1]);
    }
}
