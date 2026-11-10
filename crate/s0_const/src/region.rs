use strum::FromRepr;

/// 同城亲和度乘数（3×）
const PROX_SAME_CITY: u32 = 300;
/// 同大区亲和度乘数（2×）
const PROX_SAME_ZONE: u32 = 200;
/// 跨大区或未知地域基准乘数（1×）
const PROX_BASE: u32 = 100;
/// 地域大区百位除数（`zone = id / 100`）
const REGION_ZONE_DIV: u16 = 100;

/// 城市级地域数字常量（规避国家概念与主权争议，百位代表地理大区）
#[repr(u16)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, FromRepr)]
pub enum Region {
  /// 未知或未声明地域
  #[default]
  Unknown = 0,

  // 1xx：东亚核心城市
  Beijing = 101,
  Shanghai = 102,
  Shenzhen = 103,
  Guangzhou = 104,
  Hangzhou = 105,
  Chengdu = 106,
  HongKong = 107,
  Taipei = 108,
  Tokyo = 109,
  Osaka = 110,
  Seoul = 111,

  // 2xx：东南亚核心城市
  Singapore = 201,
  Bangkok = 202,
  Jakarta = 203,
  KualaLumpur = 204,

  // 3xx：北美核心城市
  SanFrancisco = 301,
  Seattle = 302,
  LosAngeles = 303,
  NewYork = 304,
  Chicago = 305,
  Dallas = 306,
  Toronto = 307,

  // 4xx：欧洲核心城市
  Frankfurt = 401,
  Amsterdam = 402,
  London = 403,
  Paris = 404,
  Zurich = 405,
  Stockholm = 406,

  // 5xx~8xx：大洋洲、南美、中东与非洲核心城市
  Sydney = 501,
  Melbourne = 502,
  SaoPaulo = 601,
  Dubai = 701,
  Johannesburg = 801,
}

impl Region {
  /// 从 `u16` 数值解码（未知值自动回退为 `Region::Unknown`）
  #[inline]
  pub const fn from_u16(v: u16) -> Self {
    if let Some(r) = Self::from_repr(v) {
      r
    } else {
      Self::Unknown
    }
  }

  /// 转为 `u16` 数值常量
  #[inline]
  pub const fn as_u16(self) -> u16 {
    self as u16
  }

  /// 城市三字简码（IATA 风格）
  pub const fn code(self) -> &'static str {
    match self {
      Self::Unknown => "UNK",
      Self::Beijing => "BJS",
      Self::Shanghai => "SHA",
      Self::Shenzhen => "SZX",
      Self::Guangzhou => "CAN",
      Self::Hangzhou => "HGH",
      Self::Chengdu => "CTU",
      Self::HongKong => "HKG",
      Self::Taipei => "TPE",
      Self::Tokyo => "TYO",
      Self::Osaka => "OSA",
      Self::Seoul => "SEL",
      Self::Singapore => "SIN",
      Self::Bangkok => "BKK",
      Self::Jakarta => "CGK",
      Self::KualaLumpur => "KUL",
      Self::SanFrancisco => "SFO",
      Self::Seattle => "SEA",
      Self::LosAngeles => "LAX",
      Self::NewYork => "NYC",
      Self::Chicago => "ORD",
      Self::Dallas => "DFW",
      Self::Toronto => "YYZ",
      Self::Frankfurt => "FRA",
      Self::Amsterdam => "AMS",
      Self::London => "LON",
      Self::Paris => "PAR",
      Self::Zurich => "ZRH",
      Self::Stockholm => "STO",
      Self::Sydney => "SYD",
      Self::Melbourne => "MEL",
      Self::SaoPaulo => "GRU",
      Self::Dubai => "DXB",
      Self::Johannesburg => "JNB",
    }
  }

  /// 城市中文显示名
  pub const fn name_zh(self) -> &'static str {
    match self {
      Self::Unknown => "未知",
      Self::Beijing => "北京",
      Self::Shanghai => "上海",
      Self::Shenzhen => "深圳",
      Self::Guangzhou => "广州",
      Self::Hangzhou => "杭州",
      Self::Chengdu => "成都",
      Self::HongKong => "香港",
      Self::Taipei => "台北",
      Self::Tokyo => "东京",
      Self::Osaka => "大阪",
      Self::Seoul => "首尔",
      Self::Singapore => "新加坡",
      Self::Bangkok => "曼谷",
      Self::Jakarta => "雅加达",
      Self::KualaLumpur => "吉隆坡",
      Self::SanFrancisco => "旧金山",
      Self::Seattle => "西雅图",
      Self::LosAngeles => "洛杉矶",
      Self::NewYork => "纽约",
      Self::Chicago => "芝加哥",
      Self::Dallas => "达拉斯",
      Self::Toronto => "多伦多",
      Self::Frankfurt => "法兰克福",
      Self::Amsterdam => "阿姆斯特丹",
      Self::London => "伦敦",
      Self::Paris => "巴黎",
      Self::Zurich => "苏黎世",
      Self::Stockholm => "斯德哥尔摩",
      Self::Sydney => "悉尼",
      Self::Melbourne => "墨尔本",
      Self::SaoPaulo => "圣保罗",
      Self::Dubai => "迪拜",
      Self::Johannesburg => "约翰内斯堡",
    }
  }

  /// 城市英文显示名
  const fn name_en(self) -> &'static str {
    match self {
      Self::Unknown => "Unknown",
      Self::Beijing => "Beijing",
      Self::Shanghai => "Shanghai",
      Self::Shenzhen => "Shenzhen",
      Self::Guangzhou => "Guangzhou",
      Self::Hangzhou => "Hangzhou",
      Self::Chengdu => "Chengdu",
      Self::HongKong => "HongKong",
      Self::Taipei => "Taipei",
      Self::Tokyo => "Tokyo",
      Self::Osaka => "Osaka",
      Self::Seoul => "Seoul",
      Self::Singapore => "Singapore",
      Self::Bangkok => "Bangkok",
      Self::Jakarta => "Jakarta",
      Self::KualaLumpur => "KualaLumpur",
      Self::SanFrancisco => "SanFrancisco",
      Self::Seattle => "Seattle",
      Self::LosAngeles => "LosAngeles",
      Self::NewYork => "NewYork",
      Self::Chicago => "Chicago",
      Self::Dallas => "Dallas",
      Self::Toronto => "Toronto",
      Self::Frankfurt => "Frankfurt",
      Self::Amsterdam => "Amsterdam",
      Self::London => "London",
      Self::Paris => "Paris",
      Self::Zurich => "Zurich",
      Self::Stockholm => "Stockholm",
      Self::Sydney => "Sydney",
      Self::Melbourne => "Melbourne",
      Self::SaoPaulo => "SaoPaulo",
      Self::Dubai => "Dubai",
      Self::Johannesburg => "Johannesburg",
    }
  }

  /// 计算两个对等节点之间的地域亲和度权重乘数（用于 P2P 就近节点优先调度）
  #[inline]
  pub const fn proximity_score(self, other: Self) -> u32 {
    let a = self as u16;
    let b = other as u16;
    if a == 0 || b == 0 {
      return PROX_BASE;
    }
    if a == b {
      return PROX_SAME_CITY;
    }
    if a / REGION_ZONE_DIV == b / REGION_ZONE_DIV {
      return PROX_SAME_ZONE;
    }
    PROX_BASE
  }

  /// 从城市三字码、中文名、英文名或数字字符串解析 `Region`
  pub fn parse_str(input: &str) -> Option<Self> {
    let s = input.trim();
    if let Ok(num) = s.parse::<u16>() {
      return Self::from_repr(num);
    }
    const ALL: &[Region] = &[
      Region::Unknown,
      Region::Beijing,
      Region::Shanghai,
      Region::Shenzhen,
      Region::Guangzhou,
      Region::Hangzhou,
      Region::Chengdu,
      Region::HongKong,
      Region::Taipei,
      Region::Tokyo,
      Region::Osaka,
      Region::Seoul,
      Region::Singapore,
      Region::Bangkok,
      Region::Jakarta,
      Region::KualaLumpur,
      Region::SanFrancisco,
      Region::Seattle,
      Region::LosAngeles,
      Region::NewYork,
      Region::Chicago,
      Region::Dallas,
      Region::Toronto,
      Region::Frankfurt,
      Region::Amsterdam,
      Region::London,
      Region::Paris,
      Region::Zurich,
      Region::Stockholm,
      Region::Sydney,
      Region::Melbourne,
      Region::SaoPaulo,
      Region::Dubai,
      Region::Johannesburg,
    ];
    ALL.iter().copied().find(|r| {
      r.code().eq_ignore_ascii_case(s) || r.name_zh() == s || r.name_en().eq_ignore_ascii_case(s)
    })
  }
}
