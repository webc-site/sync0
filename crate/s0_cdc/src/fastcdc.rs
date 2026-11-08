/// FastCDC 最小分块大小（16 KiB）
const CDC_MIN_CHUNK: u32 = 16 * 1024;
/// FastCDC 平均分块大小（32 KiB）
const CDC_AVG_CHUNK: u32 = 32 * 1024;
/// FastCDC 最大分块大小（64 KiB）
const CDC_MAX_CHUNK: u32 = 64 * 1024;

const WINDOW_BYTES: usize = 64;

/// 64 位 Gear 滚动哈希查找表
const GEAR64: [u64; 256] = [
  0x6517_48f5_a15f_8222,
  0xd6ed_a276_c877_d8ea,
  0x6689_6ef9_591b_326b,
  0xcd97_506b_2137_0a12,
  0x8c9c_5c9a_cbeb_2a05,
  0xb8b9_553e_e176_65ef,
  0x1784_a989_315b_1de6,
  0x9476_66c9_c50d_f4bd,
  0xb3f6_60ea_7ff2_d6a4,
  0xbcd6_adb8_d6d7_0eb5,
  0xb090_9464_f9c6_3538,
  0xe50e_3e46_a8e1_b285,
  0x21ed_7b80_c016_3ce0,
  0xf209_acd1_15f7_b43b,
  0xb8c9_cb07_eaf1_6a58,
  0xb604_78aa_97ba_854c,
  0x8fb2_13a0_b565_4c3d,
  0x42e8_e7bd_9fb0_3710,
  0x737e_3de6_0a90_b54f,
  0x9172_885f_5aa7_9c8b,
  0x787f_aae7_be10_9c36,
  0x86ad_156f_5274_cb9f,
  0x6ac0_a8da_a59e_e1ab,
  0x5e55_bc22_9d5c_618e,
  0xa54f_b69a_5f18_1d41,
  0xc433_d4cf_44d8_e974,
  0xd9ef_e85b_722e_48a3,
  0x7a5e_64f9_ea3d_9759,
  0xba37_71e1_3186_015d,
  0x5d46_8c5f_ad6e_f629,
  0x96b1_af02_152e_bfde,
  0x6370_6f4a_a70e_0111,
  0xe7a9_1692_52de_4749,
  0xf548_d625_70bc_8329,
  0xee63_9a91_17e8_c946,
  0xd31b_0f46_f3ff_6847,
  0xfed7_9384_9562_4fc5,
  0x1ef2_271c_5a28_122e,
  0x7fd8_e0e9_5eac_73ef,
  0x9205_58e0_ee13_1d4c,
  0xce2e_67cb_1034_bcd1,
  0x6f4b_338d_34b0_04ae,
  0x92f5_e727_1cf9_5c9a,
  0x12e1_305a_9c55_8342,
  0x1e30_d880_13ad_77ae,
  0x09ac_c1a5_7bbb_604e,
  0xaf18_7082_c6f5_6192,
  0xd2e5_d987_f04a_c6f0,
  0x3b22_fca4_0423_da70,
  0x7dfb_a8ce_699a_9a87,
  0xe8b1_5f90_ea96_bd2a,
  0xcda1_a108_9cc2_cbe7,
  0x72f7_0448_459d_e898,
  0x1ab9_92db_b61c_d46e,
  0x912a_d04b_ecbb_29da,
  0x98c6_bb3a_a3ce_09ed,
  0x6373_bd2e_7a04_1f3a,
  0x1f98_f28b_d178_c53a,
  0xe6ad_bc82_ba5d_9f96,
  0x7456_da7d_805c_be01,
  0xd673_662d_cc13_5eeb,
  0xb299_e26e_aadc_b311,
  0x2c25_8217_2f81_14af,
  0xeded_114d_7f62_3da6,
  0xb346_2a0e_6232_76e4,
  0x3af7_52be_3d34_bfaa,
  0x1311_ccc0_a185_5a89,
  0x0812_bbce_cc92_b2e4,
  0x9974_b574_7289_f2f5,
  0x3a03_0eff_770f_2026,
  0x5246_2b2a_a42a_847a,
  0x2bea_a107_d15a_012b,
  0x0c00_35e0_fe07_3398,
  0x4f2f_9de2_ac20_6766,
  0x5dd5_1a61_7c29_1deb,
  0x1ac6_6905_652c_c03b,
  0x1106_7b09_47fc_07a1,
  0x02b5_fcd9_6ad0_6d52,
  0x7424_4ec1_aa28_21fd,
  0xf608_9e32_060e_9439,
  0xd8f0_76a3_3bcb_f1a7,
  0x5162_743c_755d_8d5e,
  0x8d34_fc68_3e4e_3d06,
  0x46ef_e9b2_1a02_52a3,
  0x4631_e8d0_109c_6145,
  0xfdf7_a14b_c022_3957,
  0x7509_34b3_d0b8_bb1e,
  0x2ecd_1b3e_fed5_ddb9,
  0x2bcb_d89a_83cc_fbce,
  0x3507_c79e_58dd_5886,
  0x5476_a67e_cd4a_772f,
  0xaa0b_e385_6dd7_6405,
  0x2228_9a35_8a4d_d421,
  0xf570_433f_1450_3ad1,
  0x8a9f_4402_51a7_22c3,
  0x77dd_7117_52b4_398c,
  0xbbd9_edf9_c616_0a31,
  0xb94b_5922_0b23_f079,
  0xfdca_3d75_d2f3_3ccf,
  0xb294_52c4_60c9_e977,
  0xe89a_fe2d_d4bf_3b02,
  0x47ec_6f32_c91b_fee4,
  0x1aab_5ec3_4457_06b8,
  0x588b_f4fa_5533_4006,
  0xe229_0ca1_e29a_cd96,
  0x3c49_e189_f831_c37c,
  0x6448_c973_b517_7498,
  0x556a_6e09_ba15_8de7,
  0x90b2_5013_a8d9_a067,
  0xa4f2_f7a5_0c58_e1c4,
  0x5e76_5e87_1008_700e,
  0x242f_5ae7_7383_27af,
  0xc1e6_a281_9cc5_a219,
  0xcb48_d801_fd6a_5449,
  0xa208_de23_0193_1383,
  0xde3c_143f_e44e_39b0,
  0x6bb7_4b09_c73e_4133,
  0xb5b1_ed1b_63d5_4c11,
  0x5875_67d4_54ce_7716,
  0xf47d_dbc9_87cb_0392,
  0x87b1_9254_448f_03f1,
  0x985f_d00e_c372_fafa,
  0x64b9_2ba5_21aa_46e4,
  0xce63_f401_3d58_7b0f,
  0xa691_ae69_8726_030e,
  0xeaef_bf69_0264_e9aa,
  0x68ed_d400_523e_b152,
  0x35d9_353a_a195_7c60,
  0x2e2c_2d7a_9cb6_8385,
  0xfc75_49ed_af43_bf9e,
  0x48b2_adb2_3026_e2c7,
  0x3777_cb79_a024_bcf9,
  0x6441_28f7_c184_102d,
  0x7018_9d3c_a439_0de9,
  0x085f_ea79_86d4_cd34,
  0x6dbe_7626_c845_7464,
  0x9fa4_1cfa_9c42_65eb,
  0xdaa1_63a6_4194_6463,
  0x02f5_c4bd_9efa_2074,
  0x7832_0187_1822_c3c9,
  0xb0df_ec49_9202_bce0,
  0x1f1c_9c12_d84d_ccab,
  0x1596_f881_9f2e_d68e,
  0xb035_2c3e_9fc8_4468,
  0x24a6_673d_b912_2956,
  0x84f5_b9e6_0b27_4739,
  0x7216_b28a_0b54_ac46,
  0xc778_9de2_0e9c_dca4,
  0x903d_b5d2_89dd_6563,
  0xce66_a947_f703_3516,
  0x3677_dbc6_2307_b2ca,
  0x8d8e_9d55_30eb_46ac,
  0x79c4_bad2_81bd_93e2,
  0x287d_9420_4206_8c36,
  0xde4b_98e5_464b_6ad5,
  0x6125_34b9_7d1d_21bf,
  0xdf98_6597_72d8_22a1,
  0x9305_3df7_91aa_6264,
  0x2254_a8a2_d545_28ba,
  0x2301_164a_eb69_c43d,
  0xf568_6347_4ac2_417f,
  0x6136_b73e_1b75_de42,
  0xc7c3_bd48_7e06_b532,
  0x7232_fbed_1eb9_be85,
  0x36d6_0f0b_d790_9e43,
  0xe08c_bf77_4a4c_e1f2,
  0xf75f_bc0d_97cb_8384,
  0xa509_7e5a_f367_637b,
  0x7bce_2dcf_a856_dbb2,
  0xfbfb_729d_d808_c894,
  0x3dc8_eba1_0ad7_112e,
  0xf2d1_854e_edce_4928,
  0xb705_f5c1_aebd_2104,
  0x78fa_4d00_4417_d956,
  0x9e51_6266_0729_f858,
  0xda0b_cd5e_b9f9_1f0e,
  0x748d_1be1_1e06_b362,
  0xf4c2_be9a_0454_7734,
  0x6f2b_cd7c_88ab_df9a,
  0x5086_5daf_dfd8_a404,
  0x9d82_0665_6917_28f0,
  0x59fe_7a56_aa07_118e,
  0x4df1_d768_c236_60ec,
  0xab63_10b8_edfb_8c5e,
  0x029b_4762_3fc9_ffe4,
  0x50c2_cca2_3137_4860,
  0x0561_505a_8dbb_dc69,
  0x8d07_fe13_6de3_85f3,
  0xc7fb_6bb1_731b_1c1c,
  0x2496_d125_6f1f_ac7a,
  0x7950_8cee_90d8_4273,
  0x09f5_1a21_0867_6501,
  0x2ef7_2d3d_c6a5_0061,
  0xe4ad_98f5_792d_d6d6,
  0x69fa_05e6_09ae_7d33,
  0xf7f3_0a8b_9ae5_4285,
  0x04a2_cb6a_0744_764b,
  0xc4b0_762f_3967_9435,
  0x6040_1bc9_3ef6_047b,
  0x76f6_aa76_e23d_be0c,
  0x8a20_9197_811e_39da,
  0x4489_a968_3fa0_3888,
  0x2604_ad57_41a6_f8d8,
  0x7faa_9e0c_64a9_4532,
  0x0dbf_ee8c_dae8_f54e,
  0x0a7c_5885_f0b7_6d4a,
  0x55df_b1ac_12e8_3645,
  0xedc9_6765_1c49_38cc,
  0x4e00_6ab7_1a48_b85e,
  0x193f_6216_02de_413c,
  0xb564_58b7_1d56_944f,
  0xf2b6_3950_9a2f_a5da,
  0xb4a7_6f28_4c36_5450,
  0x4d3b_65d2_d2ae_22f7,
  0xbcc5_f830_3efc_a485,
  0x8a04_4f31_2671_aaea,
  0x688d_69e8_9af0_f57a,
  0x2299_57dc_1fac_ede8,
  0x2ed7_5c32_1073_da13,
  0xf199_e7ec_e5fc_efef,
  0x50c8_5b5c_837a_6c64,
  0x7170_3c6e_676b_f698,
  0xc1b4_eb52_b1e5_a518,
  0x0f46_a5e6_c9cb_68ca,
  0xebb9_3368_8d69_d7f7,
  0x5ab7_404b_8d1e_3ef4,
  0x261a_cc20_c5a6_4a90,
  0xb887_8879_8adc_718a,
  0x3e44_e9b6_bad5_bc15,
  0xf6bb_456f_0863_46bc,
  0xd66e_17e5_734c_bde1,
  0x3920_36da_e96e_389d,
  0x4a62_ceac_9d42_02de,
  0x9d55_f412_f32e_5f6e,
  0x0e1d_8415_09d9_ee9d,
  0xc313_0bdc_638e_d9e2,
  0x0cd0_e82a_f249_64d9,
  0x3ec4_c594_63ba_9b50,
  0x055b_c4d8_685a_b1bc,
  0xb9e3_43c9_6a3a_4253,
  0x8eba_190d_8688_f7f9,
  0xd31d_f36c_792c_629b,
  0xddf8_2f65_9b12_7104,
  0x6f12_dc8b_a930_fbb7,
  0xa0ae_e6bb_7e81_a7f0,
  0x8c6b_a787_47ae_8777,
  0x86f0_0167_eda1_f9bc,
  0x3a6f_8b8f_8a37_90c9,
  0x7845_bb4a_1c3b_fbbb,
  0xc875_ab07_7f66_cf23,
  0xa68b_83d8_d69b_97ee,
  0xb967_1991_39f9_a0a6,
  0x8a3a_1a4d_3de0_36b7,
  0xdf3c_5c0c_0172_32a4,
  0x8e60_e631_5699_0620,
  0xd31b_4b03_145f_02fa,
];

/// FastCDC 分块配置
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CdcConfig {
  pub min_size: usize,
  pub avg_size: usize,
  pub max_size: usize,
}

impl CdcConfig {
  pub const DEFAULT: Self = Self {
    min_size: CDC_MIN_CHUNK as usize,
    avg_size: CDC_AVG_CHUNK as usize,
    max_size: CDC_MAX_CHUNK as usize,
  };

  #[inline]
  const fn threshold(&self) -> u64 {
    let span = self
      .avg_size
      .saturating_sub(self.min_size)
      .saturating_add(1) as u64;
    u64::MAX / span
  }
}

impl Default for CdcConfig {
  fn default() -> Self {
    Self::DEFAULT
  }
}

/// 在 `data` 中查找下一个 FastCDC 内容切点
#[inline]
pub fn find_boundary(data: &[u8], cfg: CdcConfig) -> usize {
  let len = data.len().min(cfg.max_size.max(1));
  let min_size = cfg.min_size.max(1);
  if len <= min_size {
    return len;
  }

  let threshold = cfg.threshold();
  let mut rc_len = len;
  let mut rc_mask = 0u64;
  let mut hash = u64::MAX;

  let warm_start = min_size.saturating_sub(WINDOW_BYTES);
  for &b in &data[warm_start..min_size] {
    hash = (hash << 1).wrapping_add(GEAR64[usize::from(b)]);
  }

  for (off, &b) in data[min_size..len].iter().enumerate() {
    let i = min_size + off;
    if hash & rc_mask == 0 {
      if hash <= threshold {
        return i;
      }
      rc_len = i;
      rc_mask = !0u64 << (63 - hash.leading_zeros());
    }
    hash = (hash << 1).wrapping_add(GEAR64[usize::from(b)]);
  }

  if data.len() >= cfg.max_size {
    rc_len
  } else {
    len
  }
}
