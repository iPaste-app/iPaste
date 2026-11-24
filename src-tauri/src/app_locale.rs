pub(crate) const DEFAULT_LANGUAGE: &str = "en";

struct TrayText {
    open_ipaste: &'static str,
    settings: &'static str,
    quit_ipaste: &'static str,
    settings_title: &'static str,
    pause_capture: &'static str,
    resume_capture: &'static str,
    enable_append_copy: &'static str,
    disable_append_copy: &'static str,
}

const ENGLISH: TrayText = TrayText {
    open_ipaste: "Open iPaste",
    settings: "Settings...",
    quit_ipaste: "Quit iPaste",
    settings_title: "iPaste Settings",
    pause_capture: "Pause capture",
    resume_capture: "Resume capture",
    enable_append_copy: "Enable append copy",
    disable_append_copy: "Disable append copy",
};

fn tray_text(language: &str) -> Option<&'static TrayText> {
    Some(match language {
        DEFAULT_LANGUAGE => &ENGLISH,
        "zh-CN" => &TrayText {
            open_ipaste: "打开 iPaste",
            settings: "设置...",
            quit_ipaste: "退出 iPaste",
            settings_title: "iPaste 设置",
            pause_capture: "暂停捕捉",
            resume_capture: "恢复捕捉",
            enable_append_copy: "开启追加复制",
            disable_append_copy: "关闭追加复制",
        },
        "ja" => &TrayText {
            open_ipaste: "iPaste を開く",
            settings: "設定...",
            quit_ipaste: "iPaste を終了",
            settings_title: "iPaste 設定",
            pause_capture: "キャプチャを一時停止",
            resume_capture: "キャプチャを再開",
            enable_append_copy: "追記コピーを有効化",
            disable_append_copy: "追記コピーを無効化",
        },
        "ko" => &TrayText {
            open_ipaste: "iPaste 열기",
            settings: "설정...",
            quit_ipaste: "iPaste 종료",
            settings_title: "iPaste 설정",
            pause_capture: "캡처 일시 중지",
            resume_capture: "캡처 다시 시작",
            enable_append_copy: "이어붙여 복사 켜기",
            disable_append_copy: "이어붙여 복사 끄기",
        },
        "es" => &TrayText {
            open_ipaste: "Abrir iPaste",
            settings: "Ajustes...",
            quit_ipaste: "Salir de iPaste",
            settings_title: "Ajustes de iPaste",
            pause_capture: "Pausar captura",
            resume_capture: "Reanudar captura",
            enable_append_copy: "Activar copia acumulada",
            disable_append_copy: "Desactivar copia acumulada",
        },
        "fr" => &TrayText {
            open_ipaste: "Ouvrir iPaste",
            settings: "Réglages...",
            quit_ipaste: "Quitter iPaste",
            settings_title: "Réglages iPaste",
            pause_capture: "Suspendre la capture",
            resume_capture: "Reprendre la capture",
            enable_append_copy: "Activer la copie ajoutée",
            disable_append_copy: "Désactiver la copie ajoutée",
        },
        "de" => &TrayText {
            open_ipaste: "iPaste öffnen",
            settings: "Einstellungen...",
            quit_ipaste: "iPaste beenden",
            settings_title: "iPaste Einstellungen",
            pause_capture: "Erfassung pausieren",
            resume_capture: "Erfassung fortsetzen",
            enable_append_copy: "Anhängekopie aktivieren",
            disable_append_copy: "Anhängekopie deaktivieren",
        },
        "zh-TW" => &TrayText {
            open_ipaste: "開啟 iPaste",
            settings: "設定…",
            quit_ipaste: "結束 iPaste",
            settings_title: "iPaste 設定",
            pause_capture: "暫停擷取",
            resume_capture: "繼續擷取",
            enable_append_copy: "開啟追加複製",
            disable_append_copy: "關閉追加複製",
        },
        "pt-BR" => &TrayText {
            open_ipaste: "Abrir iPaste",
            settings: "Configurações…",
            quit_ipaste: "Sair do iPaste",
            settings_title: "Configurações do iPaste",
            pause_capture: "Pausar captura",
            resume_capture: "Retomar captura",
            enable_append_copy: "Ativar cópia cumulativa",
            disable_append_copy: "Desativar cópia cumulativa",
        },
        "it" => &TrayText {
            open_ipaste: "Apri iPaste",
            settings: "Impostazioni…",
            quit_ipaste: "Esci da iPaste",
            settings_title: "Impostazioni di iPaste",
            pause_capture: "Sospendi acquisizione",
            resume_capture: "Riprendi acquisizione",
            enable_append_copy: "Attiva copia cumulativa",
            disable_append_copy: "Disattiva copia cumulativa",
        },
        "ru" => &TrayText {
            open_ipaste: "Открыть iPaste",
            settings: "Настройки…",
            quit_ipaste: "Выйти из iPaste",
            settings_title: "Настройки iPaste",
            pause_capture: "Приостановить запись",
            resume_capture: "Возобновить запись",
            enable_append_copy: "Включить накопительное копирование",
            disable_append_copy: "Выключить накопительное копирование",
        },
        "pl" => &TrayText {
            open_ipaste: "Otwórz iPaste",
            settings: "Ustawienia…",
            quit_ipaste: "Zamknij iPaste",
            settings_title: "Ustawienia iPaste",
            pause_capture: "Wstrzymaj przechwytywanie",
            resume_capture: "Wznów przechwytywanie",
            enable_append_copy: "Włącz kopiowanie z dopisywaniem",
            disable_append_copy: "Wyłącz kopiowanie z dopisywaniem",
        },
        "tr" => &TrayText {
            open_ipaste: "iPaste’i aç",
            settings: "Ayarlar…",
            quit_ipaste: "iPaste’ten çık",
            settings_title: "iPaste Ayarları",
            pause_capture: "Yakalamayı duraklat",
            resume_capture: "Yakalamayı sürdür",
            enable_append_copy: "Eklemeli kopyalamayı aç",
            disable_append_copy: "Eklemeli kopyalamayı kapat",
        },
        "nl" => &TrayText {
            open_ipaste: "Open iPaste",
            settings: "Instellingen…",
            quit_ipaste: "Sluit iPaste",
            settings_title: "iPaste-instellingen",
            pause_capture: "Vastleggen pauzeren",
            resume_capture: "Vastleggen hervatten",
            enable_append_copy: "Cumulatief kopiëren inschakelen",
            disable_append_copy: "Cumulatief kopiëren uitschakelen",
        },
        "id" => &TrayText {
            open_ipaste: "Buka iPaste",
            settings: "Pengaturan…",
            quit_ipaste: "Keluar dari iPaste",
            settings_title: "Pengaturan iPaste",
            pause_capture: "Jeda perekaman",
            resume_capture: "Lanjutkan perekaman",
            enable_append_copy: "Aktifkan salin gabung",
            disable_append_copy: "Nonaktifkan salin gabung",
        },
        "vi" => &TrayText {
            open_ipaste: "Mở iPaste",
            settings: "Cài đặt…",
            quit_ipaste: "Thoát iPaste",
            settings_title: "Cài đặt iPaste",
            pause_capture: "Tạm dừng ghi nhận",
            resume_capture: "Tiếp tục ghi nhận",
            enable_append_copy: "Bật sao chép nối tiếp",
            disable_append_copy: "Tắt sao chép nối tiếp",
        },
        "th" => &TrayText {
            open_ipaste: "เปิด iPaste",
            settings: "การตั้งค่า…",
            quit_ipaste: "ออกจาก iPaste",
            settings_title: "การตั้งค่า iPaste",
            pause_capture: "พักการบันทึก",
            resume_capture: "บันทึกต่อ",
            enable_append_copy: "เปิดการคัดลอกต่อท้าย",
            disable_append_copy: "ปิดการคัดลอกต่อท้าย",
        },
        _ => return None,
    })
}

pub(crate) fn clean_language(language: String) -> Result<String, String> {
    let language = language.trim();
    if tray_text(language).is_some() {
        Ok(language.to_owned())
    } else {
        Err("Please choose a valid language".to_owned())
    }
}

pub(crate) fn localized_text(language: &str, key: &str) -> &'static str {
    let text = tray_text(language).unwrap_or(&ENGLISH);
    match key {
        "open_ipaste" => text.open_ipaste,
        "settings" => text.settings,
        "quit_ipaste" => text.quit_ipaste,
        "settings_title" => text.settings_title,
        "pause_capture" => text.pause_capture,
        "resume_capture" => text.resume_capture,
        "enable_append_copy" => text.enable_append_copy,
        "disable_append_copy" => text.disable_append_copy,
        _ => "iPaste",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Store;

    const ADDED_LANGUAGES: [&str; 10] = [
        "zh-TW", "pt-BR", "it", "ru", "pl", "tr", "nl", "id", "vi", "th",
    ];

    #[test]
    fn expanded_locales_are_accepted_and_have_native_labels() {
        for language in ADDED_LANGUAGES {
            assert_eq!(clean_language(format!(" {language} ")).unwrap(), language);
            for key in [
                "settings",
                "settings_title",
                "pause_capture",
                "resume_capture",
                "enable_append_copy",
                "disable_append_copy",
            ] {
                assert_ne!(
                    localized_text(language, key),
                    localized_text(DEFAULT_LANGUAGE, key),
                    "{language}/{key}"
                );
            }
        }
    }

    #[test]
    fn language_settings_survive_store_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("language.sqlite");
        let store = Store::new(path.clone()).unwrap();
        assert_eq!(store.settings().unwrap().language, DEFAULT_LANGUAGE);

        for language in ADDED_LANGUAGES {
            assert_eq!(
                store.update_language(language.to_owned()).unwrap().language,
                language
            );
            assert_eq!(
                Store::new(path.clone())
                    .unwrap()
                    .settings()
                    .unwrap()
                    .language,
                language
            );
        }

        assert!(store.update_language("unsupported".into()).is_err());
        assert_eq!(store.settings().unwrap().language, "th");
        assert_eq!(
            store.update_language("zh-CN".into()).unwrap().language,
            "zh-CN"
        );
    }

    #[test]
    fn unsupported_locales_fall_back_for_menu_text() {
        assert_eq!(localized_text("unknown", "settings"), ENGLISH.settings);
        assert_eq!(localized_text("unknown", "tray_tooltip"), "iPaste");
    }
}
