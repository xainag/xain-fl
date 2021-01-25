use anyhow::{Error, Result};
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use std::iter::empty;

use crate::{
    data_combination::data_points::data_point::{
        CalcScreenActiveTime,
        CalcScreenEnterCount,
        CalcWasActiveEachPastPeriod,
        CalcWasActivePastNDays,
        DataPoint,
        DataPointMetadata,
        Period,
        PeriodUnit,
    },
    database::{
        analytics_event::data_model::AnalyticsEvent,
        common::Repo,
        screen_route::data_model::ScreenRoute,
    },
};

#[allow(dead_code)]
pub struct DataCombiner<E, S> {
    events_repo: E,
    screen_routes_repo: S,
}

impl<'a, E, S> DataCombiner<E, S>
where
    E: Repo<AnalyticsEvent<'a>>,
    S: Repo<ScreenRoute>,
{
    pub fn new(events_repo: E, screen_routes_repo: S) -> Self {
        Self {
            events_repo,
            screen_routes_repo,
        }
    }

    pub fn init_data_points(&self) -> Result<Vec<DataPoint<'a>>, Error> {
        let end_period = Utc::now();
        let events = self.events_repo.get_all()?;
        let screen_routes = self.screen_routes_repo.get_all()?;

        let one_day_period_metadata =
            DataPointMetadata::new(Period::new(PeriodUnit::Days, 1), end_period);
        let was_active_each_period_metadatas = vec![
            DataPointMetadata::new(Period::new(PeriodUnit::Days, 7), end_period),
            DataPointMetadata::new(Period::new(PeriodUnit::Weeks, 6), end_period),
            DataPointMetadata::new(Period::new(PeriodUnit::Months, 3), end_period),
        ];
        let was_active_past_days_metadatas = vec![
            one_day_period_metadata,
            DataPointMetadata::new(Period::new(PeriodUnit::Days, 7), end_period),
            DataPointMetadata::new(Period::new(PeriodUnit::Days, 28), end_period),
        ];

        let data_points = empty::<DataPoint>()
            .chain(self.init_screen_active_time_vars(
                one_day_period_metadata,
                events.clone(),
                &screen_routes,
            ))
            .chain(self.init_screen_enter_count_vars(
                one_day_period_metadata,
                events.clone(),
                &screen_routes,
            ))
            .chain(self.init_was_active_each_past_period_vars(
                was_active_each_period_metadatas,
                events.clone(),
            ))
            .chain(self.init_was_active_past_n_days_vars(was_active_past_days_metadatas, events))
            .collect();
        Ok(data_points)
    }

    fn init_screen_active_time_vars(
        &self,
        metadata: DataPointMetadata,
        events: Vec<AnalyticsEvent<'a>>,
        screen_routes: &[ScreenRoute],
    ) -> Vec<DataPoint<'a>> {
        let mut screen_active_time_vars: Vec<DataPoint> = screen_routes
            .iter()
            .map(|route| {
                let events_this_route = self.get_events_single_route(&route, events.clone());
                CalcScreenActiveTime::new(
                    metadata,
                    self.filter_events_in_this_period(metadata, events_this_route),
                )
            })
            .map(DataPoint::ScreenActiveTime)
            .collect();
        screen_active_time_vars.push(DataPoint::ScreenActiveTime(CalcScreenActiveTime::new(
            metadata,
            self.filter_events_in_this_period(metadata, events),
        )));
        screen_active_time_vars
    }

    fn init_screen_enter_count_vars(
        &self,
        metadata: DataPointMetadata,
        events: Vec<AnalyticsEvent<'a>>,
        screen_routes: &[ScreenRoute],
    ) -> Vec<DataPoint<'a>> {
        screen_routes
            .iter()
            .map(|route| {
                let events_this_route = self.get_events_single_route(&route, events.clone());
                CalcScreenEnterCount::new(
                    metadata,
                    self.filter_events_in_this_period(metadata, events_this_route),
                )
            })
            .map(DataPoint::ScreenEnterCount)
            .collect()
    }

    fn init_was_active_each_past_period_vars(
        &self,
        metadatas: Vec<DataPointMetadata>,
        events: Vec<AnalyticsEvent<'a>>,
    ) -> Vec<DataPoint<'a>> {
        metadatas
            .iter()
            .map(|metadata| {
                let period_thresholds = (0..metadata.period.n)
                    .map(|i| self.get_start_of_period(*metadata, Some(i)))
                    .collect();
                CalcWasActiveEachPastPeriod::new(
                    *metadata,
                    self.filter_events_in_this_period(*metadata, events.clone()),
                    period_thresholds,
                )
            })
            .map(DataPoint::WasActiveEachPastPeriod)
            .collect()
    }

    fn init_was_active_past_n_days_vars(
        &self,
        metadatas: Vec<DataPointMetadata>,
        events: Vec<AnalyticsEvent<'a>>,
    ) -> Vec<DataPoint<'a>> {
        metadatas
            .iter()
            .map(|metadata| {
                CalcWasActivePastNDays::new(
                    *metadata,
                    self.filter_events_in_this_period(*metadata, events.clone()),
                )
            })
            .map(DataPoint::WasActivePastNDays)
            .collect()
    }

    // TODO: return an iterator instead of Vec: https://xainag.atlassian.net/browse/XN-1517
    fn filter_events_in_this_period(
        &self,
        metadata: DataPointMetadata,
        events: Vec<AnalyticsEvent<'a>>,
    ) -> Vec<AnalyticsEvent<'a>> {
        let start_of_period = self.get_start_of_period(metadata, None);
        self.filter_events_before_end_of_period(metadata.end, events)
            .into_iter()
            .filter(|event| event.timestamp > start_of_period)
            .collect()
    }

    fn get_start_of_period(
        &self,
        metadata: DataPointMetadata,
        n_periods_override: Option<u32>,
    ) -> DateTime<Utc> {
        let n_periods = if let Some(n_periods) = n_periods_override {
            n_periods
        } else {
            metadata.period.n
        };
        let avg_days_per_month = 365.0 / 12.0;
        let midnight_end_of_period = get_midnight(metadata.end);
        match metadata.period.unit {
            PeriodUnit::Days => midnight_end_of_period - Duration::days(n_periods as i64),
            PeriodUnit::Weeks => midnight_end_of_period - Duration::weeks(n_periods as i64),
            PeriodUnit::Months => {
                midnight_end_of_period
                    - Duration::days((n_periods as f64 * avg_days_per_month) as i64)
            }
        }
    }

    // TODO: return an iterator instead of Vec: https://xainag.atlassian.net/browse/XN-1517
    fn filter_events_before_end_of_period(
        &self,
        end_of_period: DateTime<Utc>,
        events: Vec<AnalyticsEvent<'a>>,
    ) -> Vec<AnalyticsEvent<'a>> {
        let midnight_end_of_period = get_midnight(end_of_period);
        events
            .into_iter()
            .filter(|event| event.timestamp < midnight_end_of_period)
            .collect()
    }

    // TODO: return an iterator instead of Vec: https://xainag.atlassian.net/browse/XN-1517
    fn get_events_single_route(
        &self,
        route: &ScreenRoute,
        all_events: Vec<AnalyticsEvent<'a>>,
    ) -> Vec<AnalyticsEvent<'a>> {
        all_events
            .into_iter()
            .filter(|event| {
                if let Some(screen_route) = event.screen_route {
                    screen_route == route
                } else {
                    false
                }
            })
            .collect()
    }
}

fn get_midnight(timestamp: DateTime<Utc>) -> DateTime<Utc> {
    DateTime::from_utc(
        NaiveDate::from_ymd(timestamp.year(), timestamp.month(), timestamp.day()).and_hms(0, 0, 0),
        Utc,
    )
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration, Utc};

    use crate::{
        data_combination::{
            data_combiner::{get_midnight, DataCombiner},
            data_points::data_point::{
                CalcScreenActiveTime,
                CalcScreenEnterCount,
                CalcWasActiveEachPastPeriod,
                CalcWasActivePastNDays,
                DataPoint,
                DataPointMetadata,
                Period,
                PeriodUnit,
            },
        },
        database::{
            analytics_event::{
                data_model::{AnalyticsEvent, AnalyticsEventType},
                repo::MockAnalyticsEventRepo,
            },
            screen_route::{data_model::ScreenRoute, repo::MockScreenRouteRepo},
        },
    };

    #[test]
    fn test_init_screen_active_time_vars() {
        let data_combiner = DataCombiner::new(MockAnalyticsEventRepo {}, MockScreenRouteRepo {});
        let end_period = DateTime::parse_from_rfc3339("2021-01-01T01:01:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 1), end_period);
        let screen_route = ScreenRoute::new("home_screen", end_period + Duration::days(1));
        let first_event = AnalyticsEvent::new(
            "test1",
            AnalyticsEventType::ScreenEnter,
            end_period - Duration::hours(12),
            Some(&screen_route),
        );
        let all_events = vec![
            first_event.clone(),
            AnalyticsEvent::new(
                "test1",
                AnalyticsEventType::AppEvent,
                end_period - Duration::hours(13),
                None,
            ),
        ];
        let expected_output = vec![
            DataPoint::ScreenActiveTime(CalcScreenActiveTime::new(metadata, vec![first_event])),
            DataPoint::ScreenActiveTime(CalcScreenActiveTime::new(metadata, all_events.clone())),
        ];
        let actual_output = data_combiner.init_screen_active_time_vars(
            metadata,
            all_events,
            &[screen_route.clone()],
        );
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_init_screen_enter_count_vars() {
        let data_combiner = DataCombiner::new(MockAnalyticsEventRepo {}, MockScreenRouteRepo {});
        let end_period = DateTime::parse_from_rfc3339("2021-02-02T02:02:00-00:00")
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
        let expected_output = vec![DataPoint::ScreenEnterCount(CalcScreenEnterCount::new(
            metadata,
            events.clone(),
        ))];
        let actual_output =
            data_combiner.init_screen_enter_count_vars(metadata, events, &[screen_route.clone()]);
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_init_was_active_each_past_period_vars() {
        let data_combiner = DataCombiner::new(MockAnalyticsEventRepo {}, MockScreenRouteRepo {});
        let end_period = DateTime::parse_from_rfc3339("2021-03-03T03:03:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 1), end_period);
        let events = vec![AnalyticsEvent::new(
            "test1",
            AnalyticsEventType::AppEvent,
            end_period - Duration::hours(12),
            None,
        )];
        let period_thresholds = vec![get_midnight(end_period)];
        let expected_output = vec![DataPoint::WasActiveEachPastPeriod(
            CalcWasActiveEachPastPeriod::new(metadata, events.clone(), period_thresholds),
        )];
        let actual_output =
            data_combiner.init_was_active_each_past_period_vars(vec![metadata], events);
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_init_was_active_past_n_days_vars() {
        let data_combiner = DataCombiner::new(MockAnalyticsEventRepo {}, MockScreenRouteRepo {});
        let end_period = DateTime::parse_from_rfc3339("2021-04-04T04:04:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 1), end_period);
        let events = vec![AnalyticsEvent::new(
            "test1",
            AnalyticsEventType::AppEvent,
            end_period - Duration::hours(12),
            None,
        )];
        let expected_output = vec![DataPoint::WasActivePastNDays(CalcWasActivePastNDays::new(
            metadata,
            events.clone(),
        ))];
        let actual_output = data_combiner.init_was_active_past_n_days_vars(vec![metadata], events);
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_filter_events_in_this_period() {
        let data_combiner = DataCombiner::new(MockAnalyticsEventRepo {}, MockScreenRouteRepo {});
        let end_period = DateTime::parse_from_rfc3339("2021-05-05T05:05:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 3), end_period);
        let event_before = AnalyticsEvent::new(
            "test1",
            AnalyticsEventType::AppEvent,
            end_period - Duration::days(5),
            None,
        );
        let event_during = AnalyticsEvent::new(
            "test2",
            AnalyticsEventType::AppEvent,
            end_period - Duration::days(1),
            None,
        );
        let event_after = AnalyticsEvent::new(
            "test3",
            AnalyticsEventType::AppEvent,
            end_period + Duration::days(2),
            None,
        );
        let events = vec![event_before, event_during.clone(), event_after];
        let expected_output = vec![event_during];
        let actual_output = data_combiner.filter_events_in_this_period(metadata, events);
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_get_start_of_period_one_day() {
        let data_combiner = DataCombiner::new(MockAnalyticsEventRepo {}, MockScreenRouteRepo {});
        let end_period = DateTime::parse_from_rfc3339("2021-01-01T00:00:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 1), end_period);
        let expected_output = end_period - Duration::days(1);
        let actual_output = data_combiner.get_start_of_period(metadata, None);
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_get_start_of_period_one_day_with_override() {
        let data_combiner = DataCombiner::new(MockAnalyticsEventRepo {}, MockScreenRouteRepo {});
        let end_period = DateTime::parse_from_rfc3339("2021-02-02T00:00:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Days, 2), end_period);
        let expected_output = end_period - Duration::days(1);
        let actual_output = data_combiner.get_start_of_period(metadata, Some(1));
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_get_start_of_period_one_week() {
        let data_combiner = DataCombiner::new(MockAnalyticsEventRepo {}, MockScreenRouteRepo {});
        let end_period = DateTime::parse_from_rfc3339("2021-03-03T00:00:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Weeks, 1), end_period);
        let expected_output = end_period - Duration::weeks(1);
        let actual_output = data_combiner.get_start_of_period(metadata, None);
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_get_start_of_period_one_month() {
        let data_combiner = DataCombiner::new(MockAnalyticsEventRepo {}, MockScreenRouteRepo {});
        let end_period = DateTime::parse_from_rfc3339("2021-04-04T00:00:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let metadata = DataPointMetadata::new(Period::new(PeriodUnit::Months, 1), end_period);
        let expected_output = end_period - Duration::days(30);
        let actual_output = data_combiner.get_start_of_period(metadata, None);
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn text_filter_events_before_end_of_period() {
        let data_combiner = DataCombiner::new(MockAnalyticsEventRepo {}, MockScreenRouteRepo {});
        let end_of_period = Utc::now();
        let event_before = AnalyticsEvent::new(
            "test1",
            AnalyticsEventType::AppEvent,
            end_of_period - Duration::days(1),
            None,
        );
        let event_after = AnalyticsEvent::new(
            "test2",
            AnalyticsEventType::AppEvent,
            end_of_period + Duration::days(1),
            None,
        );
        let events = vec![event_before.clone(), event_after];
        let expected_output = vec![event_before];
        let actual_output = data_combiner.filter_events_before_end_of_period(end_of_period, events);
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_get_events_single_route() {
        let data_combiner = DataCombiner::new(MockAnalyticsEventRepo {}, MockScreenRouteRepo {});
        let timestamp = Utc::now();
        let home_route = ScreenRoute::new("home_screen", timestamp + Duration::days(1));
        let other_route = ScreenRoute::new("other_screen", timestamp + Duration::days(2));
        let home_route_event = AnalyticsEvent::new(
            "test1",
            AnalyticsEventType::AppEvent,
            timestamp,
            Some(&home_route),
        );
        let other_route_event = AnalyticsEvent::new(
            "test2",
            AnalyticsEventType::ScreenEnter,
            timestamp,
            Some(&other_route),
        );
        let all_events = vec![home_route_event.clone(), other_route_event];
        let expected_output = vec![home_route_event];
        let actual_output = data_combiner.get_events_single_route(&home_route, all_events);
        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_get_midnight() {
        let timestamp = DateTime::parse_from_rfc3339("2021-01-01T21:21:21-02:00")
            .unwrap()
            .with_timezone(&Utc);
        let expected_output = DateTime::parse_from_rfc3339("2021-01-01T00:00:00-00:00")
            .unwrap()
            .with_timezone(&Utc);
        let actual_output = get_midnight(timestamp);
        assert_eq!(actual_output, expected_output);
    }
}
