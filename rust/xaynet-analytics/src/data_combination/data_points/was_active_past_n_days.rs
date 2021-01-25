use crate::{
    data_combination::data_points::data_point::{
        CalcWasActivePastNDays,
        CalculateDataPoints,
        DataPointMetadata,
    },
    database::analytics_event::data_model::AnalyticsEvent,
};

impl<'a> CalcWasActivePastNDays<'a> {
    pub fn new(metadata: DataPointMetadata, events: Vec<AnalyticsEvent>) -> CalcWasActivePastNDays {
        CalcWasActivePastNDays { metadata, events }
    }
}

impl<'a> CalculateDataPoints for CalcWasActivePastNDays<'a> {
    fn metadata(&self) -> DataPointMetadata {
        self.metadata
    }

    fn calculate(&self) -> Vec<u32> {
        vec![!self.events.is_empty() as u32]
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::*;
    use crate::{
        data_combination::data_points::data_point::{Period, PeriodUnit},
        database::analytics_event::data_model::AnalyticsEventType,
    };

    #[test]
    fn test_calculate_without_events() {
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 1), Utc::now());
        let was_active_past_n_days = CalcWasActivePastNDays::new(metadata, Vec::new());
        assert_eq!(was_active_past_n_days.calculate(), vec![0]);
    }

    #[test]
    fn test_calculate_with_events() {
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 1), Utc::now());
        let events = vec![AnalyticsEvent::new(
            "test1",
            AnalyticsEventType::AppEvent,
            metadata.end - Duration::hours(12),
            None,
        )];
        let was_active_past_n_days = CalcWasActivePastNDays::new(metadata, events);
        assert_eq!(was_active_past_n_days.calculate(), vec![1]);
    }
}
