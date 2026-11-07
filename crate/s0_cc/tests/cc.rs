use aok::{OK, Void};
use ctor::ctor;
use log_init::init;
use s0_cc::{
  AntiAmp, Cc, DEFAULT_AMP_FACTOR, MAX_TACK_US, MIN_TACK_US, RttEstimator, ack_interval_us,
  delay_cwnd_gain, delay_pacing_gain,
};

#[ctor(unsafe)]
fn log_init() {
  init();
}

#[test]
fn anti_amplification_and_path_validation() -> Void {
  let mut amp = AntiAmp::<u32>::default();
  assert_eq!(amp.primary(), None);
  assert_eq!(amp.allowance(), 0);
  assert!(!amp.can_send(1));

  assert!(amp.on_recv(1001, 64));
  assert_eq!(amp.primary(), Some(1001));
  assert_eq!(amp.allowance(), 64 * DEFAULT_AMP_FACTOR);
  assert!(amp.can_send(192));
  assert!(!amp.can_send(193));

  assert!(!amp.on_recv(2002, 1000));
  assert_eq!(amp.primary(), Some(1001));
  assert_eq!(amp.allowance(), 192);

  amp.on_send(150);
  assert_eq!(amp.allowance(), 42);
  assert!(!amp.can_send(50));

  amp.mark_validated();
  assert!(amp.is_validated());
  assert!(amp.can_send(1_000_000));

  amp.migrate_to(3003, 100);
  assert_eq!(amp.primary(), Some(3003));
  assert!(!amp.is_validated());
  assert_eq!(amp.allowance(), 300);
  assert!(!amp.on_recv(1001, 64));

  amp.on_send(300);
  assert!(!amp.can_send(1));
  assert!(amp.on_recv(3003, 10));
  assert_eq!(amp.allowance(), 30);
  amp.mark_validated();
  assert_eq!(amp.allowance(), u64::MAX);
  OK
}

#[test]
fn rtt_estimator_asymmetric_ewma_and_handover() -> Void {
  let mut rtt = RttEstimator::new();
  assert!(!rtt.has_rtt());

  for _ in 0..8 {
    rtt.on_sample(10_000);
  }
  assert_eq!(rtt.rtt_min_us(), 10_000);
  assert_eq!(rtt.tack_interval_us(), 2_500);

  for _ in 0..16 {
    rtt.on_sample(100_000);
  }
  assert_eq!(rtt.rtt_min_us(), 100_000);
  assert_eq!(rtt.tack_interval_us(), 20_000);

  let prev_srtt = rtt.srtt_us();
  rtt.on_sample(8_000);
  assert_eq!(rtt.rtt_min_us(), 8_000);
  assert!(rtt.srtt_us() < prev_srtt);
  OK
}

struct Link {
  t: u64,
  del: u64,
}

impl Link {
  fn run(&mut self, cc: &mut Cc, rtt: &RttEstimator, bw: u64, dur_us: u64, lost_every: u32) {
    const STEP_US: u64 = 1_000;
    for k in 1..=dur_us / STEP_US {
      self.t += STEP_US;
      self.del += bw * STEP_US / 1_000_000;
      let lost = if lost_every > 0 && k % u64::from(lost_every) == 0 {
        10
      } else {
        0
      };
      cc.on_ack(self.t, self.del, 10, lost, rtt);
    }
  }
}

#[test]
fn cc_tracks_bandwidth_and_queue() -> Void {
  let mut rtt = RttEstimator::new();
  rtt.on_sample(40_000);
  let mut cc = Cc::new();
  assert_eq!(cc.pacing_rate(&rtt), None);
  assert_eq!(cc.cwnd_bytes(&rtt), None);
  assert_eq!(cc.burst(1400, &rtt, false), 8);

  let mut link = Link { t: 0, del: 0 };
  link.run(&mut cc, &rtt, 25_000_000, 12_000, 0);
  assert!(cc.max_bw() > 20_000_000, "种子带宽 {}", cc.max_bw());

  link.run(&mut cc, &rtt, 25_000_000, 400_000, 0);
  let bw = cc.max_bw();
  assert!(bw.abs_diff(25_000_000) < 500_000, "bw={bw}");
  let rate = cc.pacing_rate(&rtt).unwrap();
  assert!(rate >= bw * 125 / 100, "rate={rate}");
  let bdp = bw * 40_000 / 1_000_000;
  let cwnd = cc.cwnd_bytes(&rtt).unwrap();
  assert!((bdp * 1375 / 1000..=bdp * 1625 / 1000).contains(&cwnd));
  assert_eq!(cc.burst(1400, &rtt, false), 4);
  assert_eq!(cc.burst(1400, &rtt, true), 32);

  link.run(&mut cc, &rtt, 10_000_000, 2_000_000, 0);
  assert!(cc.max_bw() < 15_000_000, "bw={}", cc.max_bw());

  link.run(&mut cc, &rtt, 10_000_000, 200_000, 2);
  assert!(cc.high_loss());
  let rate = cc.pacing_rate(&rtt).unwrap();
  assert_eq!(rate, cc.max_bw() * 1050 / 1000);

  let mut cc2 = Cc::new();
  assert!(!cc2.on_ack(1_000, 0, 0, 0, &rtt));
  assert!(!cc2.on_ack(10_000, 0, 1, 0, &rtt));
  assert!(cc2.on_ack(20_000, 400_000, 16, 0, &rtt));
  assert_eq!(cc2.max_bw(), 40_000_000);
  assert!(cc2.on_ack(50_000, 400_000, 16, 0, &rtt));
  assert_eq!(cc2.max_bw(), 10_000_000);
  assert!(cc2.on_ack(90_000, 720_000, 16, 0, &rtt));
  assert_eq!(cc2.max_bw(), 10_000_000);
  OK
}

#[test]
fn gain_curve_endpoints() {
  assert_eq!(delay_pacing_gain(40_000, 40_000, false, true), 1450);
  assert_eq!(delay_pacing_gain(42_000, 40_000, false, false), 1250);
  let mid = delay_pacing_gain(52_000, 40_000, false, false);
  assert!(mid > 950 && mid < 1250);
  assert_eq!(delay_pacing_gain(70_000, 40_000, false, false), 950);
  assert_eq!(delay_cwnd_gain(70_000, 40_000, false, false), 1100);
  assert_eq!(delay_pacing_gain(40_000, 40_000, true, true), 1050);
}

#[test]
fn delay_gain_monotone_and_continuous() {
  for rtt_min in [1_000u64, 40_000, 100_000, 300_000] {
    for probe in [false, true] {
      for f in [delay_pacing_gain, delay_cwnd_gain] {
        let mut prev = f(rtt_min, rtt_min, false, probe);
        for dq in (0..80_000u64).step_by(7) {
          let g = f(rtt_min + dq, rtt_min, false, probe);
          assert!(g <= prev, "rtt_min={rtt_min} dq={dq}");
          assert!(prev - g <= 1, "rtt_min={rtt_min} dq={dq} {prev}->{g}");
          prev = g;
        }
      }
    }
  }
  assert_eq!(ack_interval_us(0), 10_000);
  assert_eq!(ack_interval_us(4_000), MIN_TACK_US);
  assert_eq!(ack_interval_us(1_000_000), MAX_TACK_US);
}
