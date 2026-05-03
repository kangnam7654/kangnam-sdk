use crate::{
    self as saju, branches, daeun, daily, gongmang, interpretation, interpreter, lucky, monthly,
    shinsal, ten_gods,
    types::*,
};
use chrono::{Datelike, NaiveDate};
use serde_json::{Value, json};

pub struct SajuEngine;

impl SajuEngine {
    pub fn generate(&self, reading_type: &str, input: &Value) -> (Value, String) {
        let version = "saju-v1.0".to_string();

        match reading_type {
            "daily" => self.generate_daily(input, &version),
            "daily_detail" => self.generate_daily_detail(input, &version),
            "saju" | "saju_full" => self.generate_saju(input, &version),
            "weekly" => self.generate_weekly(input, &version),
            "monthly" => self.generate_monthly(input, &version),
            "compatibility" => self.generate_compatibility(input, &version),
            "compatibility_detail" => self.generate_compatibility_detail(input, &version),
            "monthly_fortune" => self.generate_monthly_fortune(input, &version),
            "daeun" => self.generate_daeun(input, &version),
            _ => self.generate_fallback(reading_type, input, &version),
        }
    }
}

fn parse_birth_hour(input: &str) -> Option<u32> {
    let mut parts = input.split(':');
    let hour = parts.next()?.parse::<u32>().ok()?;
    if hour > 23 {
        return None;
    }
    if let Some(minute) = parts.next() {
        let minute = minute.parse::<u32>().ok()?;
        if minute > 59 {
            return None;
        }
    }
    if parts.next().is_some() {
        return None;
    }
    Some(hour)
}

impl SajuEngine {
    /// birth_date ("YYYY-MM-DD"), birth_time ("HH:MM" or "HH") 파싱
    /// 반환: (year, month, day, hour, has_birth_time)
    fn parse_birth_data(input: &Value) -> Option<(i32, u32, u32, u32, bool)> {
        let birth_date = input.get("birth_date").and_then(|v| v.as_str())?;
        let parts: Vec<&str> = birth_date.split('-').collect();
        if parts.len() != 3 {
            return None;
        }

        let year: i32 = parts[0].parse().ok()?;
        let month: u32 = parts[1].parse().ok()?;
        let day: u32 = parts[2].parse().ok()?;
        NaiveDate::from_ymd_opt(year, month, day)?;

        let parsed_hour = match input
            .get("birth_time")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(t) => Some(parse_birth_hour(t)?),
            None => None,
        };

        let has_birth_time = parsed_hour.is_some();
        let hour = parsed_hour.unwrap_or(12); // 시간 미상이면 오시(정오) 기본값

        Some((year, month, day, hour, has_birth_time))
    }

    fn generate_daily(&self, input: &Value, version: &str) -> (Value, String) {
        let Some((year, month, day, hour, _)) = Self::parse_birth_data(input) else {
            return (
                json!({
                    "error": "생년월일 정보가 필요합니다",
                    "scores": {"overall": 75, "love": 70, "career": 75, "health": 72},
                    "advice": "프로필에 생년월일을 등록하면 더 정확한 운세를 받을 수 있습니다.",
                    "caution": "오늘도 무리하지 마세요."
                }),
                version.to_string(),
            );
        };

        let user_pillars = saju::calculate_four_pillars(year, month, day, hour);
        let fortune = daily::calculate_daily(&user_pillars);

        let result = json!({
            "date": fortune.date,
            "today_pillar": format!("{}", fortune.today_pillar),
            "day_master": format!("{} {}", fortune.day_master.korean(), fortune.day_master.element().korean()),
            "relation": fortune.relation.korean(),
            "scores": {
                "overall": fortune.scores.overall,
                "love": fortune.scores.love,
                "career": fortune.scores.career,
                "health": fortune.scores.health,
            },
            "advice": fortune.advice,
            "caution": fortune.caution,
        });

        (result, version.to_string())
    }

    fn generate_daily_detail(&self, input: &Value, version: &str) -> (Value, String) {
        let Some((year, month, day, hour, has_birth_time)) = Self::parse_birth_data(input) else {
            return (
                json!({
                    "error": "생년월일 정보가 필요합니다",
                    "scores": {"overall": 75, "love": 70, "career": 75, "health": 72, "wealth": 68},
                    "advice": "프로필에 생년월일을 등록하면 더 정확한 운세를 받을 수 있습니다.",
                    "caution": "오늘도 무리하지 마세요."
                }),
                version.to_string(),
            );
        };

        let user_pillars = saju::calculate_four_pillars(year, month, day, hour);
        let detail = daily::calculate_daily_detail(&user_pillars, has_birth_time);

        // v0.0.3 — 일간 + 가장 부족 오행 두 갈래 행운 아이템 (web /today 무료 노출용).
        let lk = lucky::analyze(&user_pillars, has_birth_time);

        let hourly: Vec<Value> = detail
            .hourly_fortunes
            .iter()
            .map(|h| {
                json!({
                    "hour_name": h.hour_name,
                    "hour_range": h.hour_range,
                    "score": h.score,
                    "description": h.description,
                })
            })
            .collect();

        let result = json!({
            "date": detail.base.date,
            "today_pillar": format!("{}", detail.base.today_pillar),
            "day_master": format!("{} {}", detail.base.day_master.korean(), detail.base.day_master.element().korean()),
            "relation": detail.base.relation.korean(),
            "scores": {
                "overall": detail.base.scores.overall,
                "love": detail.base.scores.love,
                "career": detail.base.scores.career,
                "health": detail.base.scores.health,
                "wealth": detail.category_details.wealth.score,
            },
            "advice": detail.base.advice,
            "caution": detail.base.caution,
            "category_details": {
                "love": { "score": detail.category_details.love.score, "advice": detail.category_details.love.advice },
                "career": { "score": detail.category_details.career.score, "advice": detail.category_details.career.advice },
                "health": { "score": detail.category_details.health.score, "advice": detail.category_details.health.advice },
                "wealth": { "score": detail.category_details.wealth.score, "advice": detail.category_details.wealth.advice },
            },
            "hourly_fortunes": hourly,
            "lucky_items": {
                "color": detail.lucky_items.color,
                "color_hex": detail.lucky_items.color_hex,
                "number": detail.lucky_items.number,
                "direction": detail.lucky_items.direction,
            },
            "lucky": lucky_to_json(&lk),
            "element_energy": detail.element_energy,
            "personality_summary": detail.personality_summary,
        });

        (result, version.to_string())
    }

    fn generate_saju(&self, input: &Value, version: &str) -> (Value, String) {
        let Some((year, month, day, hour, has_birth_time)) = Self::parse_birth_data(input) else {
            return (
                json!({"error": "사주 분석에는 생년월일시 정보가 필요합니다."}),
                version.to_string(),
            );
        };

        let fp = saju::calculate_four_pillars(year, month, day, hour);
        let day_master = fp.day.stem;
        let balance = ElementBalance::from_pillars_with_hour(&fp, has_birth_time);
        let gods = ten_gods::analyze_ten_gods(&fp, has_birth_time);

        let personality = interpreter::personality(day_master);
        let balance_text = interpreter::element_balance_analysis(&balance);
        let gods_text = interpreter::ten_gods_outlook(&fp, has_birth_time);

        // v0.0.3 콘텐츠 고도화 — 공망/신살/행운 아이템.
        let gm = gongmang::analyze(&fp, has_birth_time);
        let ss = shinsal::analyze(&fp, has_birth_time);
        let lk = lucky::analyze(&fp, has_birth_time);

        let gender = input
            .get("gender")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // 시주: birth_time이 있을 때만 포함
        let mut four_pillars = json!({
            "year": format!("{}", fp.year),
            "month": format!("{}", fp.month),
            "day": format!("{}", fp.day),
        });
        let mut four_pillars_detail = json!({
            "year": {"stem": day_master_info(fp.year.stem), "branch": branch_info(fp.year.branch)},
            "month": {"stem": day_master_info(fp.month.stem), "branch": branch_info(fp.month.branch)},
            "day": {"stem": day_master_info(fp.day.stem), "branch": branch_info(fp.day.branch)},
        });
        if has_birth_time {
            four_pillars
                .as_object_mut()
                .unwrap()
                .insert("hour".into(), json!(format!("{}", fp.hour)));
            four_pillars_detail.as_object_mut().unwrap()
                .insert("hour".into(), json!({"stem": day_master_info(fp.hour.stem), "branch": branch_info(fp.hour.branch)}));
        }

        // 종합 해석 — 일간·오행·십신을 합성한 5단락 + 1줄 헤드라인.
        // 카피 출처는 data/interpretation_copy.json (자평진전·적천수·궁통보감
        // ·명리정종·오행대의 정통 명리 인용 베이스). 클라이언트(웹·iOS·AI 채팅)
        // 가 같은 텍스트를 공유하도록 엔진이 들고 있는다.
        let comp = interpretation::compose(&fp, &balance, &gods);
        let interpretation_value = interpretation::to_json(&comp);

        let result = json!({
            "four_pillars": four_pillars,
            "four_pillars_detail": four_pillars_detail,
            "has_birth_time": has_birth_time,
            "day_master": {
                "stem": day_master.korean(),
                "hanja": day_master.hanja(),
                "element": day_master.element().korean(),
                "polarity": day_master.polarity().korean(),
            },
            "element_balance": {
                "wood": balance.wood,
                "fire": balance.fire,
                "earth": balance.earth,
                "metal": balance.metal,
                "water": balance.water,
                "analysis": balance_text,
            },
            "ten_gods": gods.iter().map(|(pos, god)| {
                json!({"position": pos, "god": god.korean()})
            }).collect::<Vec<_>>(),
            "personality": personality,
            "fortune_outlook": gods_text,
            "gender": gender,
            "gongmang": gongmang_to_json(&gm),
            "shinsal": ss.iter().map(shinsal_to_json).collect::<Vec<_>>(),
            "lucky": lucky_to_json(&lk),
            "interpretation": interpretation_value,
        });

        (result, version.to_string())
    }

    fn generate_weekly(&self, input: &Value, version: &str) -> (Value, String) {
        let Some((year, month, day, hour, _)) = Self::parse_birth_data(input) else {
            return (
                json!({"error": "생년월일 정보가 필요합니다"}),
                version.to_string(),
            );
        };

        let user_pillars = saju::calculate_four_pillars(year, month, day, hour);
        let kst = chrono::FixedOffset::east_opt(9 * 3600).unwrap();
        let today = chrono::Utc::now().with_timezone(&kst).date_naive();

        let days: Vec<Value> = (0..7)
            .map(|offset| {
                let date = today + chrono::Duration::days(offset);
                let fortune = daily::calculate_daily_for_date(
                    &user_pillars,
                    date.year(),
                    date.month(),
                    date.day(),
                );
                json!({
                    "date": date.format("%Y-%m-%d").to_string(),
                    "scores": {
                        "overall": fortune.scores.overall,
                        "love": fortune.scores.love,
                        "career": fortune.scores.career,
                        "health": fortune.scores.health,
                    },
                    "advice": fortune.advice,
                    "grade": score_to_grade(fortune.scores.overall),
                })
            })
            .collect();

        let avg_score = days
            .iter()
            .filter_map(|d| {
                d.get("scores")
                    .and_then(|s| s.get("overall"))
                    .and_then(|v| v.as_i64())
            })
            .sum::<i64>()
            / 7;

        let result = json!({
            "period": format!("{} ~ {}", today.format("%Y-%m-%d"), (today + chrono::Duration::days(6)).format("%Y-%m-%d")),
            "average_score": avg_score,
            "days": days,
            "summary": format!("이번 주 평균 운세 점수는 {}점입니다.", avg_score),
        });

        (result, version.to_string())
    }

    fn generate_monthly(&self, input: &Value, version: &str) -> (Value, String) {
        let Some((year, month, day, hour, _)) = Self::parse_birth_data(input) else {
            return (
                json!({"error": "생년월일 정보가 필요합니다"}),
                version.to_string(),
            );
        };

        let user_pillars = saju::calculate_four_pillars(year, month, day, hour);
        let kst = chrono::FixedOffset::east_opt(9 * 3600).unwrap();
        let now = chrono::Utc::now().with_timezone(&kst).date_naive();
        let target_year = now.year();
        let target_month = now.month();
        let total_days = days_in_month(target_year, target_month);

        // 주간별 요약 (4~5주)
        let mut weeks: Vec<Value> = Vec::new();
        let mut week_scores: Vec<i32> = Vec::new();
        let mut all_scores: Vec<i32> = Vec::new();

        for d in 1..=total_days {
            let fortune =
                daily::calculate_daily_for_date(&user_pillars, target_year, target_month, d);
            all_scores.push(fortune.scores.overall);
            week_scores.push(fortune.scores.overall);

            if week_scores.len() == 7 || d == total_days {
                let avg = week_scores.iter().sum::<i32>() / week_scores.len() as i32;
                weeks.push(json!({
                    "week": weeks.len() + 1,
                    "average_score": avg,
                    "grade": score_to_grade(avg),
                }));
                week_scores.clear();
            }
        }

        let monthly_avg = all_scores.iter().sum::<i32>() / all_scores.len() as i32;
        let best_day = all_scores
            .iter()
            .enumerate()
            .max_by_key(|(_, s)| *s)
            .map(|(i, _)| i + 1)
            .unwrap_or(1);
        let worst_day = all_scores
            .iter()
            .enumerate()
            .min_by_key(|(_, s)| *s)
            .map(|(i, _)| i + 1)
            .unwrap_or(1);

        let result = json!({
            "year": target_year,
            "month": target_month,
            "average_score": monthly_avg,
            "grade": score_to_grade(monthly_avg),
            "best_day": best_day,
            "worst_day": worst_day,
            "weeks": weeks,
            "summary": format!("{}월 평균 운세 점수는 {}점입니다. 가장 좋은 날은 {}일, 주의할 날은 {}일입니다.", target_month, monthly_avg, best_day, worst_day),
        });

        (result, version.to_string())
    }

    fn generate_compatibility(&self, input: &Value, version: &str) -> (Value, String) {
        let Some(compat) = Self::compute_compatibility(input) else {
            return (
                json!({"error": "생년월일 정보가 필요합니다"}),
                version.to_string(),
            );
        };
        (compat.to_basic_json(), version.to_string())
    }

    fn generate_compatibility_detail(&self, input: &Value, version: &str) -> (Value, String) {
        let Some(compat) = Self::compute_compatibility(input) else {
            return (
                json!({"error": "생년월일 정보가 필요합니다"}),
                version.to_string(),
            );
        };
        (compat.to_detail_json(), version.to_string())
    }

    fn generate_monthly_fortune(&self, input: &Value, version: &str) -> (Value, String) {
        let Some((birth_year, birth_month, birth_day, birth_hour, _)) =
            Self::parse_birth_data(input)
        else {
            return (
                json!({
                    "error": "생년월일 정보가 필요합니다",
                    "year": 0,
                    "current_month": 0,
                    "current_month_summary": {
                        "score": 75,
                        "grade": "good",
                        "advice": "프로필에 생년월일을 등록하면 더 정확한 월운을 받을 수 있습니다."
                    },
                    "months": []
                }),
                version.to_string(),
            );
        };

        let user_pillars =
            saju::calculate_four_pillars(birth_year, birth_month, birth_day, birth_hour);

        let kst = chrono::FixedOffset::east_opt(9 * 3600).unwrap();
        let now = chrono::Utc::now().with_timezone(&kst).date_naive();

        // options.year 우선, 없으면 현재 KST 연도
        let year = input
            .get("options")
            .and_then(|o| o.get("year"))
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| now.year() as i64) as i32;

        let current_month = now.month();

        let months_data = monthly::calculate_monthly_fortune(&user_pillars, year);

        // 이번 달 summary (current_month가 요청 연도에 속할 때만 유효)
        let current_summary = months_data
            .iter()
            .find(|m| m.month == current_month && year == now.year())
            .or_else(|| months_data.first())
            .map(|m| {
                json!({
                    "score": m.score,
                    "grade": m.grade,
                    "advice": m.advice,
                })
            })
            .unwrap_or_else(|| {
                json!({
                    "score": 75,
                    "grade": "good",
                    "advice": "이번 달도 꾸준히 나아가세요.",
                })
            });

        let months_json: Vec<Value> = months_data
            .iter()
            .map(|m| {
                json!({
                    "month": m.month,
                    "score": m.score,
                    "grade": m.grade,
                    "categories": {
                        "overall": m.categories.overall,
                        "love": m.categories.love,
                        "career": m.categories.career,
                        "health": m.categories.health,
                        "wealth": m.categories.wealth,
                    },
                    "advice": m.advice,
                })
            })
            .collect();

        let result = json!({
            "year": year,
            "current_month": current_month,
            "current_month_summary": current_summary,
            "months": months_json,
        });

        (result, version.to_string())
    }

    fn generate_daeun(&self, input: &Value, version: &str) -> (Value, String) {
        let Some((birth_year, birth_month, birth_day, birth_hour, _)) =
            Self::parse_birth_data(input)
        else {
            return (
                json!({
                    "error": "생년월일 정보가 필요합니다",
                    "periods": [],
                    "current_period_index": null
                }),
                version.to_string(),
            );
        };

        let gender = input.get("gender").and_then(|v| v.as_str()).unwrap_or("M");

        let user_pillars =
            saju::calculate_four_pillars(birth_year, birth_month, birth_day, birth_hour);

        let periods =
            daeun::calculate_daeun(&user_pillars, birth_year, birth_month, birth_day, gender);

        let current_period_index: Option<usize> = periods.iter().position(|p| p.is_current);

        let periods_json: Vec<Value> = periods
            .iter()
            .map(|p| {
                json!({
                    "start_age": p.start_age,
                    "end_age": p.end_age,
                    "stem": p.stem,
                    "branch": p.branch,
                    "element": p.element,
                    "score": p.score,
                    "description": p.description,
                    "is_current": p.is_current,
                })
            })
            .collect();

        let result = json!({
            "periods": periods_json,
            "current_period_index": current_period_index,
        });

        (result, version.to_string())
    }

    fn generate_fallback(
        &self,
        reading_type: &str,
        _input: &Value,
        version: &str,
    ) -> (Value, String) {
        // compatibility 등 아직 미구현 타입은 기본 응답
        let result = json!({
            "reading_type": reading_type,
            "summary": "이 기능은 준비 중입니다.",
            "score": 75,
            "advice": "곧 더 정확한 분석을 제공해 드리겠습니다.",
        });
        (result, version.to_string())
    }
}

/// 궁합 분석 중간 결과
struct CompatibilityData {
    score: i32,
    grade: &'static str,
    analysis: String,
    love: i32,
    communication: i32,
    values: i32,
    lifestyle: i32,
    subject_info: Value,
    target_info: Value,
    balance1: ElementBalance,
    balance2: ElementBalance,
    branch_analysis: branches::BranchAnalysis,
    ten_god_interactions: Vec<(TenGod, TenGod, i32)>,
}

impl CompatibilityData {
    fn to_basic_json(&self) -> Value {
        json!({
            "score": self.score,
            "grade": self.grade,
            "analysis": self.analysis,
            "categories": {
                "love": self.love,
                "communication": self.communication,
                "values": self.values,
                "lifestyle": self.lifestyle,
            },
            "subject_info": self.subject_info,
            "target_info": self.target_info,
        })
    }

    fn to_detail_json(&self) -> Value {
        let branch_relations: Vec<Value> = self.build_branch_relations();
        let ten_god_list: Vec<Value> = self
            .ten_god_interactions
            .iter()
            .map(|(sg, tg, _)| {
                json!({
                    "subject_god": sg.korean(),
                    "target_god": tg.korean(),
                    "interpretation": ten_god_interaction_text(*sg, *tg),
                })
            })
            .collect();

        let weakest = self.balance1.weakest();
        let lucky_color = element_to_color(weakest);
        let lucky_direction = element_to_direction(weakest);

        json!({
            "score": self.score,
            "grade": self.grade,
            "analysis": self.analysis,
            "categories": {
                "love": {
                    "score": self.love,
                    "analysis": category_analysis("love", self.love, &self.branch_analysis),
                    "advice": category_advice("love", self.love),
                },
                "communication": {
                    "score": self.communication,
                    "analysis": category_analysis("communication", self.communication, &self.branch_analysis),
                    "advice": category_advice("communication", self.communication),
                },
                "values": {
                    "score": self.values,
                    "analysis": category_analysis("values", self.values, &self.branch_analysis),
                    "advice": category_advice("values", self.values),
                },
                "lifestyle": {
                    "score": self.lifestyle,
                    "analysis": category_analysis("lifestyle", self.lifestyle, &self.branch_analysis),
                    "advice": category_advice("lifestyle", self.lifestyle),
                },
            },
            "element_comparison": {
                "subject": {
                    "wood": self.balance1.wood,
                    "fire": self.balance1.fire,
                    "earth": self.balance1.earth,
                    "metal": self.balance1.metal,
                    "water": self.balance1.water,
                },
                "target": {
                    "wood": self.balance2.wood,
                    "fire": self.balance2.fire,
                    "earth": self.balance2.earth,
                    "metal": self.balance2.metal,
                    "water": self.balance2.water,
                },
            },
            "branch_relations": branch_relations,
            "ten_god_interactions": ten_god_list,
            "advice": {
                "overall": compatibility_advice_detailed(self.score, "overall"),
                "caution": compatibility_advice_detailed(self.score, "caution"),
                "enhancement": format!("{}의 기운이 부족하니 {} 소품을 활용해보세요.", weakest.korean(), lucky_color),
            },
            "lucky_elements": {
                "color": lucky_color,
                "element": weakest.korean(),
                "direction": lucky_direction,
            },
            "subject_info": self.subject_info,
            "target_info": self.target_info,
        })
    }

    fn build_branch_relations(&self) -> Vec<Value> {
        let mut relations = Vec::new();
        for s in &self.branch_analysis.samhap {
            let (label, desc) = match s {
                branches::SamhapResult::Full(e) => (
                    format!("삼합(三合) - {}국", e.korean()),
                    "세 가지 기운이 하나로 모여 강한 조화를 이룹니다.".to_string(),
                ),
                branches::SamhapResult::Half(e) => (
                    format!("반합(半合) - {}국", e.korean()),
                    "부분적인 조화가 있어 서로 보완합니다.".to_string(),
                ),
            };
            relations.push(
                json!({"type": label, "branches": [], "effect": "positive", "description": desc}),
            );
        }
        for y in &self.branch_analysis.yukhap {
            relations.push(json!({
                "type": "육합(六合)",
                "branches": [y.pair.0.korean(), y.pair.1.korean()],
                "effect": "positive",
                "description": "자연스러운 끌림과 조화의 관계입니다.",
            }));
        }
        for c in &self.branch_analysis.clashes {
            relations.push(json!({
                "type": "상충(相沖)",
                "branches": [c.pair.0.korean(), c.pair.1.korean()],
                "effect": "negative",
                "description": "서로 다른 에너지가 부딪혀 갈등이 생길 수 있습니다.",
            }));
        }
        for p in &self.branch_analysis.punishments {
            let br_names: Vec<&str> = p.branches.iter().map(|b| b.korean()).collect();
            relations.push(json!({
                "type": p.punishment_type.korean(),
                "branches": br_names,
                "effect": "negative",
                "description": "관계에서 미묘한 갈등 요소가 있습니다.",
            }));
        }
        relations
    }
}

impl SajuEngine {
    /// 궁합 핵심 계산 (기본/상세 공용)
    fn compute_compatibility(input: &Value) -> Option<CompatibilityData> {
        let (year1, month1, day1, hour1, _) = Self::parse_birth_data(input)?;
        let pillars1 = saju::calculate_four_pillars(year1, month1, day1, hour1);
        let day_master1 = pillars1.day.stem;
        let has_hour1 = input.get("birth_time").and_then(|v| v.as_str()).is_some();
        let balance1 = ElementBalance::from_pillars_with_hour(&pillars1, has_hour1);

        let target_date = input
            .get("options")
            .and_then(|o| o.get("target_birth_date"))
            .and_then(|v| v.as_str())?;
        let parts: Vec<&str> = target_date.split('-').collect();
        if parts.len() != 3 {
            return None;
        }

        let y2: i32 = parts[0].parse().ok()?;
        let m2: u32 = parts[1].parse().ok()?;
        let d2: u32 = parts[2].parse().ok()?;
        let h2 = input
            .get("options")
            .and_then(|o| o.get("target_birth_time"))
            .and_then(|v| v.as_str())
            .and_then(|t| t.split(':').next()?.parse::<u32>().ok())
            .unwrap_or(12);
        let has_hour2 = input
            .get("options")
            .and_then(|o| o.get("target_birth_time"))
            .and_then(|v| v.as_str())
            .is_some();

        let pillars2 = saju::calculate_four_pillars(y2, m2, d2, h2);
        let day_master2 = pillars2.day.stem;
        let balance2 = ElementBalance::from_pillars_with_hour(&pillars2, has_hour2);

        // 1. 오행 보완 점수
        let elem_score = calculate_compatibility_score(&balance1, &balance2);
        let rel = saju::elements::relation(day_master1.element(), day_master2.element());
        let rel_bonus = match rel {
            saju::elements::ElementRelation::Generated => 15,
            saju::elements::ElementRelation::Generates => 10,
            saju::elements::ElementRelation::Same => 5,
            saju::elements::ElementRelation::Controls => -5,
            saju::elements::ElementRelation::Controlled => -10,
        };
        let base_score = (elem_score + rel_bonus).clamp(30, 98);

        // 2. 지지 관계 분석
        let subj_branches = collect_branches(&pillars1, has_hour1);
        let tgt_branches = collect_branches(&pillars2, has_hour2);
        let branch_analysis = branches::analyze(&subj_branches, &tgt_branches);

        // 3. 십신 상호작용
        let ten_god_interactions =
            analyze_cross_ten_gods(&pillars1, &pillars2, has_hour1, has_hour2);
        let ten_god_bonus: i32 = ten_god_interactions.iter().map(|(_, _, s)| s).sum();

        // 4. 카테고리별 독립 점수
        let love = (base_score
            + branch_analysis.yukhap_count as i32 * 8
            + branch_analysis.samhap_count as i32 * 6
            - branch_analysis.sangchung_count as i32 * 4)
            .clamp(30, 98);

        // 천간 상생/상극 카운트
        let (sangsaeng, sanggeuk) =
            count_stem_relations(&pillars1, &pillars2, has_hour1, has_hour2);
        let communication = (base_score + sangsaeng as i32 * 6 - sanggeuk as i32 * 4).clamp(30, 98);

        let values = (base_score + ten_god_bonus).clamp(30, 98);

        let balance_diff: i32 = [
            (balance1.wood as i32 - balance2.wood as i32).abs(),
            (balance1.fire as i32 - balance2.fire as i32).abs(),
            (balance1.earth as i32 - balance2.earth as i32).abs(),
            (balance1.metal as i32 - balance2.metal as i32).abs(),
            (balance1.water as i32 - balance2.water as i32).abs(),
        ]
        .iter()
        .sum();
        let complement_bonus = if balance_diff <= 5 { 3 } else { 0 };
        let lifestyle = (base_score - balance_diff * 2 + complement_bonus).clamp(30, 98);

        // 종합 점수 = 카테고리 가중 평균
        let score =
            ((love * 30 + communication * 25 + values * 25 + lifestyle * 20) / 100).clamp(30, 98);
        let grade = score_to_grade(score);

        let analysis = format!(
            "{}({})과 {}({})의 궁합입니다. {}",
            day_master1.korean(),
            day_master1.element().korean(),
            day_master2.korean(),
            day_master2.element().korean(),
            compatibility_advice(score),
        );

        let target_name = input
            .get("options")
            .and_then(|o| o.get("target_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("상대방");

        let subject_info = json!({
            "name": "나",
            "day_master": day_master1.korean(),
            "element": day_master1.element().korean(),
            "animal": pillars1.year.branch.animal(),
        });
        let target_info = json!({
            "name": target_name,
            "day_master": day_master2.korean(),
            "element": day_master2.element().korean(),
            "animal": pillars2.year.branch.animal(),
        });

        Some(CompatibilityData {
            score,
            grade,
            analysis,
            love,
            communication,
            values,
            lifestyle,
            subject_info,
            target_info,
            balance1,
            balance2,
            branch_analysis,
            ten_god_interactions,
        })
    }
}

fn collect_branches(pillars: &FourPillars, include_hour: bool) -> Vec<Branch> {
    let mut v = vec![
        pillars.year.branch,
        pillars.month.branch,
        pillars.day.branch,
    ];
    if include_hour {
        v.push(pillars.hour.branch);
    }
    v
}

fn collect_stems(pillars: &FourPillars, include_hour: bool) -> Vec<Stem> {
    let mut v = vec![pillars.year.stem, pillars.month.stem, pillars.day.stem];
    if include_hour {
        v.push(pillars.hour.stem);
    }
    v
}

fn analyze_cross_ten_gods(
    p1: &FourPillars,
    p2: &FourPillars,
    has_hour1: bool,
    has_hour2: bool,
) -> Vec<(TenGod, TenGod, i32)> {
    let dm1 = p1.day.stem;
    let dm2 = p2.day.stem;
    let stems2 = collect_stems(p2, has_hour2);
    let stems1 = collect_stems(p1, has_hour1);

    let my_gods: Vec<TenGod> = stems2
        .iter()
        .map(|&s| ten_gods::derive_ten_god(dm1, s))
        .collect();
    let their_gods: Vec<TenGod> = stems1
        .iter()
        .map(|&s| ten_gods::derive_ten_god(dm2, s))
        .collect();

    let mut interactions = Vec::new();
    let bonus_pairs: &[(TenGod, TenGod, i32)] = &[
        (TenGod::Jeonggwan, TenGod::Jeongjae, 5),
        (TenGod::Sikshin, TenGod::Pyeonin, 4),
        (TenGod::Jeongin, TenGod::Jeonggwan, 4),
        (TenGod::Bigyeon, TenGod::Bigyeon, 3),
    ];
    let penalty_pairs: &[(TenGod, TenGod, i32)] = &[
        (TenGod::Sanggwan, TenGod::Pyeongwan, -5),
        (TenGod::Geupjae, TenGod::Pyeonjae, -4),
        (TenGod::Sanggwan, TenGod::Jeonggwan, -3),
    ];

    for &mg in &my_gods {
        for &tg in &their_gods {
            for &(a, b, score) in bonus_pairs {
                if (mg == a && tg == b) || (mg == b && tg == a) {
                    interactions.push((mg, tg, score));
                }
            }
            for &(a, b, score) in penalty_pairs {
                if (mg == a && tg == b) || (mg == b && tg == a) {
                    interactions.push((mg, tg, score));
                }
            }
        }
    }
    interactions.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
    interactions
}

fn count_stem_relations(p1: &FourPillars, p2: &FourPillars, h1: bool, h2: bool) -> (usize, usize) {
    let stems1 = collect_stems(p1, h1);
    let stems2 = collect_stems(p2, h2);
    let mut sangsaeng = 0usize;
    let mut sanggeuk = 0usize;
    for &s1 in &stems1 {
        for &s2 in &stems2 {
            if saju::elements::generates(s1.element(), s2.element())
                || saju::elements::generates(s2.element(), s1.element())
            {
                sangsaeng += 1;
            }
            if saju::elements::controls(s1.element(), s2.element())
                || saju::elements::controls(s2.element(), s1.element())
            {
                sanggeuk += 1;
            }
        }
    }
    (sangsaeng, sanggeuk)
}

fn ten_god_interaction_text(a: TenGod, b: TenGod) -> &'static str {
    match (a, b) {
        (TenGod::Jeonggwan, TenGod::Jeongjae) | (TenGod::Jeongjae, TenGod::Jeonggwan) => {
            "안정적인 관계 기반이 됩니다."
        }
        (TenGod::Sikshin, TenGod::Pyeonin) | (TenGod::Pyeonin, TenGod::Sikshin) => {
            "창의적 에너지가 교류됩니다."
        }
        (TenGod::Jeongin, TenGod::Jeonggwan) | (TenGod::Jeonggwan, TenGod::Jeongin) => {
            "지적 교감이 깊습니다."
        }
        (TenGod::Bigyeon, TenGod::Bigyeon) => "동질감이 강합니다.",
        (TenGod::Sanggwan, TenGod::Pyeongwan) | (TenGod::Pyeongwan, TenGod::Sanggwan) => {
            "권위 충돌이 발생할 수 있습니다."
        }
        (TenGod::Geupjae, TenGod::Pyeonjae) | (TenGod::Pyeonjae, TenGod::Geupjae) => {
            "재물 관련 갈등이 있을 수 있습니다."
        }
        (TenGod::Sanggwan, TenGod::Jeonggwan) | (TenGod::Jeonggwan, TenGod::Sanggwan) => {
            "관계에서 마찰이 생길 수 있습니다."
        }
        _ => "독특한 상호작용이 있습니다.",
    }
}

fn element_to_color(e: Element) -> &'static str {
    match e {
        Element::Wood => "초록색",
        Element::Fire => "빨간색",
        Element::Earth => "노란색",
        Element::Metal => "흰색",
        Element::Water => "파란색",
    }
}

fn element_to_direction(e: Element) -> &'static str {
    match e {
        Element::Wood => "동쪽",
        Element::Fire => "남쪽",
        Element::Earth => "중앙",
        Element::Metal => "서쪽",
        Element::Water => "북쪽",
    }
}

fn category_analysis(category: &str, score: i32, ba: &branches::BranchAnalysis) -> String {
    match category {
        "love" if ba.yukhap_count > 0 => format!(
            "지지에 육합이 {}개 발견되어 자연스러운 끌림이 있습니다.",
            ba.yukhap_count
        ),
        "love" if ba.samhap_count > 0 => "삼합의 조화로 깊은 유대감을 형성합니다.".to_string(),
        "love" => format!("연애 궁합 점수는 {}점입니다.", score),
        "communication" if score >= 80 => "천간의 상생 관계가 많아 소통이 원활합니다.".into(),
        "communication" if score >= 60 => "대화를 통해 이해를 넓힐 수 있는 관계입니다.".into(),
        "communication" => "소통에 노력이 필요한 관계입니다.".into(),
        "values" if score >= 80 => "십신 조합이 조화로워 가치관이 잘 맞습니다.".into(),
        "values" if score >= 60 => "서로 다른 관점이 보완이 되는 관계입니다.".into(),
        "values" => "가치관 차이를 인정하고 존중하는 것이 중요합니다.".into(),
        "lifestyle" if score >= 80 => {
            "오행 밸런스가 상호보완적이어서 함께하면 안정적입니다.".into()
        }
        "lifestyle" if score >= 60 => "생활 방식에서 적절한 균형을 찾을 수 있습니다.".into(),
        "lifestyle" => "생활 습관 차이를 조율하는 노력이 필요합니다.".into(),
        _ => format!("점수: {}점", score),
    }
}

fn category_advice(category: &str, score: i32) -> &'static str {
    match (category, score) {
        ("love", 80..=98) => "서로의 감정을 솔직하게 표현하면 더욱 깊어집니다.",
        ("love", 60..=79) => "작은 관심과 배려가 관계를 한층 발전시킵니다.",
        ("love", 40..=59) => "서로의 사랑 표현 방식을 이해하려 노력하세요.",
        ("love", _) => "감정 표현에 더 적극적으로 다가가 보세요.",
        ("communication", 80..=98) => "열린 대화를 유지하면 더욱 단단해집니다.",
        ("communication", 60..=79) => "상대의 의견을 경청하는 시간을 가지세요.",
        ("communication", 40..=59) => "오해를 줄이기 위해 명확한 표현을 연습하세요.",
        ("communication", _) => "대화의 기회를 의식적으로 만들어 보세요.",
        ("values", 80..=98) => "장기적 목표를 함께 논의하면 시너지가 납니다.",
        ("values", 60..=79) => "서로의 우선순위를 존중하며 공통점을 찾으세요.",
        ("values", 40..=59) => "차이를 인정하고 타협점을 찾아보세요.",
        ("values", _) => "서로의 세계관을 이해하려는 노력이 필요합니다.",
        ("lifestyle", 80..=98) => "주말 활동을 함께 계획하면 유대가 깊어집니다.",
        ("lifestyle", 60..=79) => "각자의 시간과 함께하는 시간의 균형을 맞추세요.",
        ("lifestyle", 40..=59) => "생활 패턴의 차이를 조율하는 규칙을 만들어 보세요.",
        ("lifestyle", _) => "서로의 생활 방식을 존중하는 것이 우선입니다.",
        _ => "서로를 이해하려는 노력이 중요합니다.",
    }
}

fn compatibility_advice_detailed(score: i32, advice_type: &str) -> &'static str {
    match (advice_type, score) {
        ("overall", 90..=98) => "천생연분에 가까운 궁합입니다. 서로를 더욱 성장시킬 수 있습니다.",
        ("overall", 80..=89) => "서로의 부족한 부분을 잘 채워주는 훌륭한 궁합입니다.",
        ("overall", 70..=79) => "안정적이고 편안한 관계를 유지할 수 있습니다.",
        ("overall", 60..=69) => "노력하면 좋은 관계로 발전할 수 있는 궁합입니다.",
        ("overall", 50..=59) => "서로 이해하려는 노력이 필요하지만 가능성이 있습니다.",
        ("overall", 40..=49) => "차이를 인정하고 존중하면 성장할 수 있습니다.",
        ("overall", _) => "서로 다른 성향이 강하므로 소통과 양보가 중요합니다.",
        ("caution", 80..=98) => "좋은 궁합이지만 서로를 당연시하지 않도록 주의하세요.",
        ("caution", 60..=79) => "작은 갈등이 쌓이지 않도록 정기적으로 대화하세요.",
        ("caution", 40..=59) => "감정적 충돌 시 한 발 물러서는 여유를 가지세요.",
        ("caution", _) => "서로의 차이를 비난하지 말고 이해하려 노력하세요.",
        _ => "",
    }
}

fn day_master_info(stem: Stem) -> Value {
    json!({
        "korean": stem.korean(),
        "hanja": stem.hanja(),
        "element": stem.element().korean(),
    })
}

fn branch_info(branch: Branch) -> Value {
    json!({
        "korean": branch.korean(),
        "hanja": branch.hanja(),
        "animal": branch.animal(),
        "element": branch.element().korean(),
    })
}

/// 공망 결과를 web 친화 JSON으로. enum은 한국어/lowercase 키로 평탄화.
fn gongmang_to_json(g: &gongmang::Gongmang) -> Value {
    let palaces: Vec<&'static str> = g
        .affected_palaces
        .iter()
        .map(|p| match p {
            gongmang::Palace::Year => "year",
            gongmang::Palace::Month => "month",
            gongmang::Palace::Hour => "hour",
        })
        .collect();
    let ten_gods: Vec<&'static str> = g.affected_ten_gods.iter().map(|t| t.korean()).collect();

    json!({
        "group_index": g.group_index,
        "group_name": g.group_name,
        "empty_branches": g.empty_branches.iter().map(|b| branch_info(*b)).collect::<Vec<_>>(),
        "affected_palaces": palaces,
        "affected_ten_gods": ten_gods,
        "interpretation": g.interpretation,
    })
}

/// 단일 신살 → JSON. kind는 영문 슬러그 + 한국어 라벨 둘 다 노출.
fn shinsal_to_json(s: &shinsal::Shinsal) -> Value {
    let (kind_slug, kind_korean) = match s.kind {
        shinsal::ShinsalKind::Geop => ("geop", "겁살"),
        shinsal::ShinsalKind::Jae => ("jae", "재살"),
        shinsal::ShinsalKind::Cheon => ("cheon", "천살"),
        shinsal::ShinsalKind::Ji => ("ji", "지살"),
        shinsal::ShinsalKind::Dohwa => ("dohwa", "도화살"),
        shinsal::ShinsalKind::Wol => ("wol", "월살"),
        shinsal::ShinsalKind::Mangsin => ("mangsin", "망신살"),
        shinsal::ShinsalKind::Jangseong => ("jangseong", "장성살"),
        shinsal::ShinsalKind::Banan => ("banan", "반안살"),
        shinsal::ShinsalKind::Yeokma => ("yeokma", "역마살"),
        shinsal::ShinsalKind::Yukae => ("yukae", "육해살"),
        shinsal::ShinsalKind::Hwagae => ("hwagae", "화개살"),
        shinsal::ShinsalKind::Baekho => ("baekho", "백호살"),
        shinsal::ShinsalKind::Cheoneul => ("cheoneul", "천을귀인"),
    };
    let positions: Vec<&'static str> = s
        .positions
        .iter()
        .map(|p| match p {
            shinsal::ShinsalPosition::Year => "year",
            shinsal::ShinsalPosition::Month => "month",
            shinsal::ShinsalPosition::Day => "day",
            shinsal::ShinsalPosition::Hour => "hour",
        })
        .collect();

    json!({
        "kind": kind_slug,
        "kind_korean": kind_korean,
        "positions": positions,
        "intensity": s.intensity,
        "modern_take": s.modern_take,
    })
}

/// 행운 아이템 → 한국어 오행 라벨 포함 JSON.
fn lucky_to_json(l: &lucky::LuckyItems) -> Value {
    json!({
        "primary": lucky_triple_to_json(&l.primary),
        "supplementary": lucky_triple_to_json(&l.supplementary),
        "interpretation": l.interpretation,
    })
}

fn lucky_triple_to_json(t: &lucky::LuckyTriple) -> Value {
    json!({
        "element": t.element.korean(),
        "color": t.color,
        "numbers": t.numbers,
        "direction": t.direction,
    })
}

/// 해당 월의 일 수 계산
fn days_in_month(year: i32, month: u32) -> u32 {
    // 다음 달 1일에서 하루를 빼면 이번 달 마지막 날
    let next_month_year = if month == 12 { year + 1 } else { year };
    let next_month = if month == 12 { 1 } else { month + 1 };
    chrono::NaiveDate::from_ymd_opt(next_month_year, next_month, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(30)
}

/// 오행 밸런스 상호보완 점수 계산
fn calculate_compatibility_score(b1: &ElementBalance, b2: &ElementBalance) -> i32 {
    // 서로의 약한 오행을 보완해주는 정도 계산
    let elements = [
        (b1.wood, b2.wood),
        (b1.fire, b2.fire),
        (b1.earth, b2.earth),
        (b1.metal, b2.metal),
        (b1.water, b2.water),
    ];

    let mut complement_score = 0i32;
    for (a, b) in &elements {
        let diff = (*a as i32 - *b as i32).abs();
        // 차이가 적당하면 상호보완 → 높은 점수
        complement_score += match diff {
            0..=1 => 14,
            2..=3 => 12,
            4..=5 => 10,
            _ => 6,
        };
    }

    complement_score.clamp(30, 98)
}

fn compatibility_advice(score: i32) -> &'static str {
    match score {
        90..=98 => "천생연분에 가까운 궁합입니다. 서로를 더욱 빛나게 합니다.",
        85..=89 => "서로의 부족한 부분을 잘 채워주는 훌륭한 궁합입니다.",
        80..=84 => "안정적이고 조화로운 관계를 기대할 수 있습니다.",
        75..=79 => "서로 맞춰가면 좋은 관계로 발전할 수 있습니다.",
        70..=74 => "무난한 궁합이지만 작은 노력으로 더 좋아질 수 있습니다.",
        60..=69 => "차이가 있지만 서로 이해하면 성장할 수 있는 관계입니다.",
        50..=59 => "성격 차이가 있을 수 있으나 이해와 배려로 극복 가능합니다.",
        40..=49 => "서로 다른 점이 많지만 그만큼 배울 수 있는 관계입니다.",
        _ => "서로 다른 성향이 강하므로 소통과 양보가 중요합니다.",
    }
}

/// 점수를 등급 문자열로 변환
fn score_to_grade(score: i32) -> &'static str {
    match score {
        80..=i32::MAX => "great",
        60..=79 => "good",
        40..=59 => "normal",
        _ => "caution",
    }
}

#[cfg(test)]
mod content_depth_tests {
    //! v0.0.3 콘텐츠 고도화 — saju/daily_detail JSON 응답에 신규 필드 anchor 잠금.
    //! 다운스트림(lunawave web) UI가 의지하는 키 이름이 무심코 사라지지 않게 한다.

    use super::*;
    use serde_json::json;

    fn saju_input_with_time() -> Value {
        json!({
            "birth_date": "1990-05-15",
            "birth_time": "14:00",
            "gender": "male",
        })
    }

    #[test]
    fn saju_response_includes_gongmang_shinsal_lucky() {
        let (result, _v) = SajuEngine.generate("saju", &saju_input_with_time());

        let gm = result.get("gongmang").expect("gongmang 필드 필수");
        assert!(gm.get("group_index").is_some(), "gongmang.group_index");
        assert!(gm.get("group_name").is_some(), "gongmang.group_name");
        assert!(
            gm.get("empty_branches")
                .and_then(|v| v.as_array())
                .is_some_and(|a| a.len() == 2),
            "공망 지지는 항상 2개"
        );
        assert!(
            gm.get("interpretation").is_some(),
            "gongmang.interpretation"
        );

        let ss = result
            .get("shinsal")
            .and_then(|v| v.as_array())
            .expect("shinsal은 배열");
        // 신살은 0개일 수 있으나 배열 자체는 항상 존재.
        for item in ss {
            assert!(item.get("kind").is_some(), "각 신살에 kind 슬러그");
            assert!(item.get("kind_korean").is_some(), "각 신살에 kind_korean");
            assert!(item.get("positions").is_some(), "각 신살에 positions");
            assert!(item.get("modern_take").is_some(), "각 신살에 modern_take");
        }

        let lk = result.get("lucky").expect("lucky 필드 필수");
        let primary = lk.get("primary").expect("lucky.primary");
        assert!(primary.get("element").is_some());
        assert!(primary.get("color").is_some());
        assert!(
            primary
                .get("numbers")
                .and_then(|v| v.as_array())
                .is_some_and(|a| a.len() == 2),
            "행운 숫자 2개"
        );
        assert!(primary.get("direction").is_some());
        assert!(lk.get("supplementary").is_some());
        assert!(lk.get("interpretation").is_some());
    }

    #[test]
    fn daily_detail_response_includes_new_lucky_alongside_legacy() {
        let (result, _v) = SajuEngine.generate("daily_detail", &saju_input_with_time());

        // 레거시 lucky_items는 그대로 — iOS 호환.
        let legacy = result.get("lucky_items").expect("legacy lucky_items 유지");
        assert!(legacy.get("color").is_some());
        assert!(legacy.get("color_hex").is_some());
        assert!(legacy.get("number").is_some());

        // 신규 lucky (primary + supplementary).
        let new_lucky = result.get("lucky").expect("신규 lucky 필드");
        assert!(new_lucky.get("primary").is_some());
        assert!(new_lucky.get("supplementary").is_some());
    }

    #[test]
    fn shinsal_kind_slugs_are_lowercase_english() {
        let (result, _v) = SajuEngine.generate("saju", &saju_input_with_time());
        let ss = result.get("shinsal").and_then(|v| v.as_array()).unwrap();
        let allowed = [
            "geop",
            "jae",
            "cheon",
            "ji",
            "dohwa",
            "wol",
            "mangsin",
            "jangseong",
            "banan",
            "yeokma",
            "yukae",
            "hwagae",
            "baekho",
            "cheoneul",
        ];
        for item in ss {
            let k = item.get("kind").and_then(|v| v.as_str()).unwrap();
            assert!(allowed.contains(&k), "예상치 못한 신살 슬러그: {}", k);
        }
    }

    #[test]
    fn gongmang_palaces_are_lowercase() {
        let (result, _v) = SajuEngine.generate("saju", &saju_input_with_time());
        let palaces = result
            .get("gongmang")
            .and_then(|g| g.get("affected_palaces"))
            .and_then(|v| v.as_array())
            .unwrap();
        let allowed = ["year", "month", "hour"];
        for p in palaces {
            let s = p.as_str().unwrap();
            assert!(allowed.contains(&s), "예상치 못한 궁 슬러그: {}", s);
        }
    }

    #[test]
    fn invalid_birth_date_returns_error_instead_of_fallback_chart() {
        let (result, _v) = SajuEngine.generate(
            "saju",
            &json!({
                "birth_date": "2024-02-31",
                "birth_time": "14:00",
            }),
        );

        assert!(result.get("error").is_some());
        assert!(result.get("four_pillars").is_none());
    }

    #[test]
    fn invalid_birth_time_returns_error_instead_of_wrapping_hour() {
        let (result, _v) = SajuEngine.generate(
            "daily",
            &json!({
                "birth_date": "2024-02-29",
                "birth_time": "24:00",
            }),
        );

        assert!(result.get("error").is_some());
    }
}
