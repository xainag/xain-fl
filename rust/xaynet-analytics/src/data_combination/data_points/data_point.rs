use chrono::{DateTime, Utc};

use crate::database::analytics_event::data_model::AnalyticsEvent;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PeriodUnit {
    Days,
    Weeks,
    Months,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Period {
    pub unit: PeriodUnit,
    pub n: u32,
}

impl Period {
    pub fn new(unit: PeriodUnit, n: u32) -> Period {
        Period { unit, n }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct DataPointMetadata {
    pub period: Period,
    pub end: DateTime<Utc>,
}

impl DataPointMetadata {
    pub fn new(period: Period, end: DateTime<Utc>) -> DataPointMetadata {
        DataPointMetadata { period, end }
    }
}

pub trait CalculateDataPoints {
    fn metadata(&self) -> DataPointMetadata;

    fn calculate(&self) -> Vec<u32>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum DataPoint<'a> {
    ScreenActiveTime(CalcScreenActiveTime<'a>),
    ScreenEnterCount(CalcScreenEnterCount<'a>),
    WasActiveEachPastPeriod(CalcWasActiveEachPastPeriod<'a>),
    WasActivePastNDays(CalcWasActivePastNDays<'a>),
}

#[allow(dead_code)]
// TODO: will be called when preparing the data to be sent to the coordinator
impl<'a> DataPoint<'a> {
    fn calculate(&self) -> Vec<u32> {
        match self {
            DataPoint::ScreenActiveTime(data) => data.calculate(),
            DataPoint::ScreenEnterCount(data) => data.calculate(),
            DataPoint::WasActiveEachPastPeriod(data) => data.calculate(),
            DataPoint::WasActivePastNDays(data) => data.calculate(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
// TODO: accept an iterator instead of Vec: https://xainag.atlassian.net/browse/XN-1517
pub struct CalcScreenActiveTime<'a> {
    pub metadata: DataPointMetadata,
    pub events: Vec<AnalyticsEvent<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
// TODO: accept an iterator instead of Vec: https://xainag.atlassian.net/browse/XN-1517
pub struct CalcScreenEnterCount<'a> {
    pub metadata: DataPointMetadata,
    pub events: Vec<AnalyticsEvent<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
// TODO: accept an iterator instead of Vec: https://xainag.atlassian.net/browse/XN-1517
pub struct CalcWasActiveEachPastPeriod<'a> {
    pub metadata: DataPointMetadata,
    pub events: Vec<AnalyticsEvent<'a>>,
    pub period_thresholds: Vec<DateTime<Utc>>,
}

#[derive(Debug, PartialEq, Eq)]
// TODO: accept an iterator instead of Vec: https://xainag.atlassian.net/browse/XN-1517
pub struct CalcWasActivePastNDays<'a> {
    pub metadata: DataPointMetadata,
    pub events: Vec<AnalyticsEvent<'a>>,
}
