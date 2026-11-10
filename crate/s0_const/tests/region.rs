use aok::{OK, Void};
use ctor::ctor;
use log_init::init;
use s0_const::region::Region;

#[ctor(unsafe)]
fn _log_init() {
  init();
}

#[test]
fn test_region_proximity_and_lookup() -> Void {
  assert_eq!(Region::parse_str("SHA"), Some(Region::Shanghai));
  assert_eq!(Region::parse_str("深圳"), Some(Region::Shenzhen));
  assert_eq!(Region::parse_str("shanghai"), Some(Region::Shanghai));
  assert_eq!(Region::parse_str("108"), Some(Region::Taipei));
  assert_eq!(Region::parse_str("9999"), None);

  assert_eq!(Region::Shanghai.proximity_score(Region::Shanghai), 300);
  assert_eq!(Region::Shanghai.proximity_score(Region::Tokyo), 200);
  assert_eq!(Region::Shanghai.proximity_score(Region::Frankfurt), 100);
  assert_eq!(Region::Shanghai.proximity_score(Region::Unknown), 100);
  OK
}
