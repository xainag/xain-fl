use anyhow::{Error, Result};

use crate::database::{
    analytics_event::data_model::AnalyticsEvent,
    common::{IsarAdapter, Repo},
    isar::IsarDb,
};

pub struct AnalyticsEventRepo<'r> {
    collection_name: &'r str,
    db: &'r IsarDb,
}

impl<'r> AnalyticsEventRepo<'r> {
    pub fn new(collection_name: &'r str, db: &'r IsarDb) -> Self {
        Self {
            collection_name,
            db,
        }
    }
}

impl<'r, 'a> Repo<AnalyticsEvent<'a>> for AnalyticsEventRepo<'r> {
    fn add(&self, event: &mut AnalyticsEvent) -> Result<(), Error> {
        let mut object_builder = self.db.get_object_builder(self.collection_name)?;
        event.write_with_object_builder(&mut object_builder);
        self.db
            .put(&self.collection_name, object_builder.finish().as_bytes())
            .map(|_| ())
    }

    // TODO: return an iterator instead of Vec: https://xainag.atlassian.net/browse/XN-1517
    fn get_all(&self) -> Result<Vec<AnalyticsEvent<'a>>, Error> {
        let _events_as_bytes = self.db.get_all_as_bytes(&self.collection_name)?;

        // TODO: not sure how to proceed to parse [u8] using the collection schema. didn't find examples in Isar
        // implement when possible: https://xainag.atlassian.net/browse/XN-1604
        unimplemented!()
    }
}

pub struct MockAnalyticsEventRepo {}

impl<'a> Repo<AnalyticsEvent<'a>> for MockAnalyticsEventRepo {
    fn add(&self, _object: &mut AnalyticsEvent) -> Result<(), Error> {
        Ok(())
    }

    fn get_all(&self) -> Result<Vec<AnalyticsEvent<'a>>, Error> {
        Ok(Vec::new())
    }
}
