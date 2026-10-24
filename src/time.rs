//! Time and date helpers using Win32 local time APIs.
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::System::Time::{
    FileTimeToSystemTime, SystemTimeToFileTime, SystemTimeToTzSpecificLocalTime,
    TzSpecificLocalTimeToSystemTime,
};

const EPOCH_DIFF_100NS: u64 = 116_444_736_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub weekday: u32, // 0 = Sunday, 1 = Monday, ... 6 = Saturday
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn local_now() -> LocalTime {
    unsafe {
        let st = GetLocalTime();
        LocalTime {
            year: st.wYear as i32,
            month: st.wMonth as u32,
            day: st.wDay as u32,
            weekday: st.wDayOfWeek as u32,
            hour: st.wHour as u32,
            minute: st.wMinute as u32,
            second: st.wSecond as u32,
        }
    }
}

pub fn ms_to_filetime(ms: i64) -> FILETIME {
    let t = if ms >= 0 {
        (ms as u64) * 10_000 + EPOCH_DIFF_100NS
    } else {
        0
    };
    FILETIME {
        dwLowDateTime: (t & 0xFFFF_FFFF) as u32,
        dwHighDateTime: (t >> 32) as u32,
    }
}

pub fn filetime_to_ms(ft: FILETIME) -> i64 {
    let t = ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64);
    if t >= EPOCH_DIFF_100NS {
        ((t - EPOCH_DIFF_100NS) / 10_000) as i64
    } else {
        0
    }
}

/// Convert unix epoch ms to local date-time.
pub fn local_from_ms(ms: i64) -> LocalTime {
    unsafe {
        let ft = ms_to_filetime(ms);
        let mut utc_st = SYSTEMTIME::default();
        let mut loc_st = SYSTEMTIME::default();
        if FileTimeToSystemTime(&ft, &mut utc_st).is_ok()
            && SystemTimeToTzSpecificLocalTime(None, &utc_st, &mut loc_st).is_ok()
        {
            LocalTime {
                year: loc_st.wYear as i32,
                month: loc_st.wMonth as u32,
                day: loc_st.wDay as u32,
                weekday: loc_st.wDayOfWeek as u32,
                hour: loc_st.wHour as u32,
                minute: loc_st.wMinute as u32,
                second: loc_st.wSecond as u32,
            }
        } else {
            local_now()
        }
    }
}

/// Convert local date-time components back to unix epoch ms.
pub fn ms_from_local(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> i64 {
    unsafe {
        let loc_st = SYSTEMTIME {
            wYear: year as u16,
            wMonth: month as u16,
            wDayOfWeek: 0,
            wDay: day as u16,
            wHour: hour as u16,
            wMinute: minute as u16,
            wSecond: second as u16,
            wMilliseconds: 0,
        };
        let mut utc_st = SYSTEMTIME::default();
        let mut ft = FILETIME::default();
        if TzSpecificLocalTimeToSystemTime(None, &loc_st, &mut utc_st).is_ok()
            && SystemTimeToFileTime(&utc_st, &mut ft).is_ok()
        {
            filetime_to_ms(ft)
        } else {
            now_ms()
        }
    }
}

pub fn day_key(ms: i64) -> String {
    let t = local_from_ms(ms);
    format!("{:04}-{:02}-{:02}", t.year, t.month, t.day)
}

pub fn day_key_now() -> String {
    let t = local_now();
    format!("{:04}-{:02}-{:02}", t.year, t.month, t.day)
}

/// Format: "{wd}, {d} {mon}" e.g. "Sab, 24 Okt".
pub fn id_date_short(year: i32, month: u32, day: u32, weekday: u32) -> String {
    let _ = year;
    const WD: [&str; 7] = ["Min", "Sen", "Sel", "Rab", "Kam", "Jum", "Sab"];
    const MON: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des",
    ];
    let wd = WD.get(weekday as usize).unwrap_or(&"");
    let mon = MON
        .get(month.saturating_sub(1) as usize)
        .unwrap_or(&"");
    format!("{wd}, {day} {mon}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_date_short() {
        assert_eq!(id_date_short(2026, 10, 24, 6), "Sab, 24 Okt");
    }

    #[test]
    fn test_day_key() {
        let t = local_now();
        let k = day_key_now();
        assert_eq!(k, format!("{:04}-{:02}-{:02}", t.year, t.month, t.day));
    }
}
