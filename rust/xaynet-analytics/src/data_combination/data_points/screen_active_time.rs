use chrono::Duration;

use crate::{
    data_combination::data_points::data_point::{
        CalcScreenActiveTime,
        CalculateDataPoints,
        DataPointMetadata,
    },
    database::analytics_event::data_model::{AnalyticsEvent, AnalyticsEventType},
};

impl<'a> CalcScreenActiveTime<'a> {
    pub fn new(metadata: DataPointMetadata, events: Vec<AnalyticsEvent>) -> CalcScreenActiveTime {
        CalcScreenActiveTime { metadata, events }
    }

    // TODO: return an iterator instead of Vec: https://xainag.atlassian.net/browse/XN-1517
    fn get_screen_and_app_events(&self) -> Vec<AnalyticsEvent> {
        self.events
            .iter()
            .filter(|event| {
                matches!(
                    event.event_type,
                    AnalyticsEventType::ScreenEnter | AnalyticsEventType::AppEvent
                )
            })
            .cloned()
            .collect()
    }
}

impl<'a> CalculateDataPoints for CalcScreenActiveTime<'a> {
    fn metadata(&self) -> DataPointMetadata {
        self.metadata
    }

    fn calculate(&self) -> Vec<u32> {
        let screen_and_app_events = self.get_screen_and_app_events();
        let value = if screen_and_app_events.is_empty() {
            0
        } else {
            screen_and_app_events
                .iter()
                .scan(
                    screen_and_app_events.first().unwrap().timestamp,
                    |last_timestamp, event| {
                        let duration = if event.event_type == AnalyticsEventType::ScreenEnter {
                            last_timestamp.signed_duration_since(event.timestamp)
                        } else {
                            Duration::zero()
                        };
                        *last_timestamp = event.timestamp;
                        Some(duration)
                    },
                )
                .map(|duration| duration.num_milliseconds() as u32)
                .sum()
        };
        vec![value]
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration, Utc};

    use super::*;
    use crate::{
        data_combination::data_points::data_point::{Period, PeriodUnit},
        database::screen_route::data_model::ScreenRoute,
    };

    #[test]
    fn test_get_screen_and_app_events() {
        let end_period = DateTime::parse_from_rfc3339("2021-01-01T01:01:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 1), end_period);
        let screen_route = ScreenRoute::new("home_screen", end_period + Duration::days(1));
        let screen_enter_event = AnalyticsEvent::new(
            "test1",
            AnalyticsEventType::ScreenEnter,
            end_period - Duration::hours(10),
            Some(&screen_route),
        );
        let app_event = AnalyticsEvent::new(
            "test1",
            AnalyticsEventType::AppEvent,
            end_period - Duration::hours(12),
            None,
        );
        let events = vec![
            screen_enter_event.clone(),
            AnalyticsEvent::new(
                "test1",
                AnalyticsEventType::Error,
                end_period - Duration::hours(11),
                None,
            ),
            app_event.clone(),
            AnalyticsEvent::new(
                "test1",
                AnalyticsEventType::UserAction,
                end_period - Duration::hours(13),
                None,
            ),
        ];
        let screen_active_time = CalcScreenActiveTime::new(metadata, events);
        let expected_output = vec![screen_enter_event, app_event];
        let actual_output = screen_active_time.get_screen_and_app_events();
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_calculate_when_no_events() {
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 1), Utc::now());
        let screen_active_time = CalcScreenActiveTime::new(metadata, Vec::new());
        assert_eq!(screen_active_time.calculate(), vec![0]);
    }

    #[test]
    fn test_calculate_when_one_screen_enter_event() {
        let end_period = DateTime::parse_from_rfc3339("2021-03-03T03:03:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 1), end_period);
        let screen_route = ScreenRoute::new("home_screen", end_period + Duration::days(1));
        let events = vec![AnalyticsEvent::new(
            "test1",
            AnalyticsEventType::ScreenEnter,
            end_period - Duration::hours(12),
            Some(&screen_route),
        )];
        let screen_active_time = CalcScreenActiveTime::new(metadata, events);
        assert_eq!(screen_active_time.calculate(), vec![0]);
    }

    #[test]
    fn test_calculate_when_two_screen_enter_events() {
        let end_period = DateTime::parse_from_rfc3339("2021-03-03T03:03:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 1), end_period);
        let screen_route = ScreenRoute::new("home_screen", end_period + Duration::days(1));
        let events = vec![
            AnalyticsEvent::new(
                "test1",
                AnalyticsEventType::ScreenEnter,
                end_period - Duration::hours(12),
                Some(&screen_route),
            ),
            AnalyticsEvent::new(
                "test2",
                AnalyticsEventType::ScreenEnter,
                end_period - Duration::hours(15),
                Some(&screen_route),
            ),
        ];
        let time_between_events =
            events.first().unwrap().timestamp - events.last().unwrap().timestamp;
        let screen_active_time = CalcScreenActiveTime::new(metadata, events);
        assert_eq!(
            screen_active_time.calculate(),
            vec![time_between_events.num_milliseconds() as u32]
        );
    }

    #[test]
    fn test_calculate_when_mixed_type_events() {
        let end_period = DateTime::parse_from_rfc3339("2021-04-04T04:04:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 1), end_period);
        let screen_route = ScreenRoute::new("home_screen", end_period + Duration::days(1));
        let first = AnalyticsEvent::new(
            "test1",
            AnalyticsEventType::ScreenEnter,
            end_period - Duration::hours(12),
            Some(&screen_route),
        );
        let second = AnalyticsEvent::new(
            "test1",
            AnalyticsEventType::AppEvent,
            end_period - Duration::hours(13),
            None,
        );
        let third = AnalyticsEvent::new(
            "test2",
            AnalyticsEventType::ScreenEnter,
            end_period - Duration::hours(14),
            Some(&screen_route),
        );
        let fourth = AnalyticsEvent::new(
            "test2",
            AnalyticsEventType::ScreenEnter,
            end_period - Duration::hours(14),
            Some(&screen_route),
        );
        let events = vec![first.clone(), second.clone(), third.clone(), fourth.clone()];
        let time_between_events =
            first.timestamp - second.timestamp + (third.timestamp - fourth.timestamp);
        let screen_active_time = CalcScreenActiveTime::new(metadata, events);
        assert_eq!(
            screen_active_time.calculate(),
            vec![time_between_events.num_milliseconds() as u32]
        );
    }
}
