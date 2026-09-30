use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use glib::Variant;
use glib::prelude::*;

const MPRIS_XML: &str = r#"
<node>
  <interface name="org.mpris.MediaPlayer2">
    <method name="Raise"/>
    <method name="Quit"/>
    <property name="CanQuit" type="b" access="read"/>
    <property name="CanRaise" type="b" access="read"/>
    <property name="HasTrackList" type="b" access="read"/>
    <property name="Identity" type="s" access="read"/>
    <property name="DesktopEntry" type="s" access="read"/>
    <property name="SupportedUriSchemes" type="as" access="read"/>
    <property name="SupportedMimeTypes" type="as" access="read"/>
  </interface>
  <interface name="org.mpris.MediaPlayer2.Player">
    <method name="Next"/>
    <method name="Previous"/>
    <method name="Pause"/>
    <method name="PlayPause"/>
    <method name="Stop"/>
    <method name="Play"/>
    <method name="Seek">
      <arg direction="in" name="Offset" type="x"/>
    </method>
    <method name="SetPosition">
      <arg direction="in" name="TrackId" type="o"/>
      <arg direction="in" name="Position" type="x"/>
    </method>
    <method name="OpenUri">
      <arg direction="in" name="Uri" type="s"/>
    </method>
    <signal name="Seeked">
      <arg name="Position" type="x"/>
    </signal>
    <property name="PlaybackStatus" type="s" access="read"/>
    <property name="Rate" type="d" access="readwrite"/>
    <property name="Metadata" type="a{sv}" access="read"/>
    <property name="Volume" type="d" access="readwrite"/>
    <property name="Position" type="x" access="read"/>
    <property name="MinimumRate" type="d" access="read"/>
    <property name="MaximumRate" type="d" access="read"/>
    <property name="CanGoNext" type="b" access="read"/>
    <property name="CanGoPrevious" type="b" access="read"/>
    <property name="CanPlay" type="b" access="read"/>
    <property name="CanPause" type="b" access="read"/>
    <property name="CanSeek" type="b" access="read"/>
    <property name="CanControl" type="b" access="read"/>
  </interface>
</node>
"#;

#[derive(Debug, Clone)]
pub enum MprisCommand {
    Raise,
    Quit,
    Next,
    Previous,
    Pause,
    PlayPause,
    Stop,
    Play,
    SeekRelativeUs(i64),
    SetPositionUs(i64),
    OpenUri(String),
    SetVolume(f64),
    SetRate(f64),
}

#[derive(Debug, Clone)]
struct MprisState {
    playback_status: String,
    rate: f64,
    volume: f64,
    position_us: i64,
    duration_us: i64,
    title: String,
    url: String,
    connection: Option<gio::DBusConnection>,
}

impl Default for MprisState {
    fn default() -> Self {
        Self {
            playback_status: "Stopped".to_string(),
            rate: 1.0,
            volume: 1.0,
            position_us: 0,
            duration_us: 0,
            title: "Omaplay".to_string(),
            url: String::new(),
            connection: None,
        }
    }
}

#[derive(Clone)]
pub struct MprisServer {
    state: Rc<RefCell<MprisState>>,
    _owner_id: Rc<gio::OwnerId>,
}

impl MprisServer {
    pub fn start<F>(on_command: F) -> Self
    where
        F: Fn(MprisCommand) + 'static,
    {
        let state = Rc::new(RefCell::new(MprisState::default()));
        let on_cmd = Rc::new(on_command);

        let state_bus = Rc::clone(&state);
        let cmd_bus = Rc::clone(&on_cmd);

        let owner_id = gio::bus_own_name(
            gio::BusType::Session,
            "org.mpris.MediaPlayer2.omaplay",
            gio::BusNameOwnerFlags::REPLACE,
            move |connection, _name| {
                state_bus.borrow_mut().connection = Some(connection.clone());
                let Ok(node_info) = gio::DBusNodeInfo::for_xml(MPRIS_XML) else {
                    return;
                };

                if let Some(root_iface) = node_info.lookup_interface("org.mpris.MediaPlayer2") {
                    let cmd_root = Rc::clone(&cmd_bus);
                    let _ = connection
                        .register_object("/org/mpris/MediaPlayer2", &root_iface)
                        .method_call(
                            move |_conn, _sender, _path, _iface, method, _params, invocation| {
                                match method {
                                    "Raise" => cmd_root(MprisCommand::Raise),
                                    "Quit" => cmd_root(MprisCommand::Quit),
                                    _ => {}
                                }
                                invocation.return_value(None);
                            },
                        )
                        .property(
                            move |_conn, _sender, _path, _iface, prop_name| match prop_name {
                                "CanQuit" | "CanRaise" => true.to_variant(),
                                "HasTrackList" => false.to_variant(),
                                "Identity" => "Omaplay".to_variant(),
                                "DesktopEntry" => "omaplay".to_variant(),
                                "SupportedUriSchemes" => {
                                    vec!["file", "http", "https"].to_variant()
                                }
                                "SupportedMimeTypes" => vec![
                                    "video/mp4",
                                    "video/x-matroska",
                                    "video/webm",
                                    "audio/mpeg",
                                    "audio/flac",
                                ]
                                .to_variant(),
                                _ => false.to_variant(),
                            },
                        )
                        .build();
                }

                if let Some(player_iface) =
                    node_info.lookup_interface("org.mpris.MediaPlayer2.Player")
                {
                    let cmd_player = Rc::clone(&cmd_bus);
                    let cmd_set_prop = Rc::clone(&cmd_bus);
                    let st_prop = Rc::clone(&state_bus);

                    let _ = connection
                        .register_object("/org/mpris/MediaPlayer2", &player_iface)
                        .method_call(
                            move |_conn, _sender, _path, _iface, method, params, invocation| {
                                match method {
                                    "Next" => cmd_player(MprisCommand::Next),
                                    "Previous" => cmd_player(MprisCommand::Previous),
                                    "Pause" => cmd_player(MprisCommand::Pause),
                                    "PlayPause" => cmd_player(MprisCommand::PlayPause),
                                    "Stop" => cmd_player(MprisCommand::Stop),
                                    "Play" => cmd_player(MprisCommand::Play),
                                    "Seek" => {
                                        if let Some((offset_us,)) = params.try_get::<(i64,)>().ok()
                                        {
                                            cmd_player(MprisCommand::SeekRelativeUs(offset_us));
                                        }
                                    }
                                    "SetPosition" => {
                                        if let Some((_track_id, pos_us)) =
                                            params.try_get::<(glib::Variant, i64)>().ok()
                                        {
                                            cmd_player(MprisCommand::SetPositionUs(pos_us));
                                        }
                                    }
                                    "OpenUri" => {
                                        if let Some((uri,)) = params.try_get::<(String,)>().ok() {
                                            cmd_player(MprisCommand::OpenUri(uri));
                                        }
                                    }
                                    _ => {}
                                }
                                invocation.return_value(None);
                            },
                        )
                        .property(
                            move |_conn, _sender, _path, _iface, prop_name| {
                                let st = st_prop.borrow();
                                match prop_name {
                                    "PlaybackStatus" => st.playback_status.to_variant(),
                                    "Rate" => st.rate.to_variant(),
                                    "Metadata" => build_metadata_variant(&st),
                                    "Volume" => st.volume.to_variant(),
                                    "Position" => st.position_us.to_variant(),
                                    "MinimumRate" => 0.1f64.to_variant(),
                                    "MaximumRate" => 8.0f64.to_variant(),
                                    "CanGoNext"
                                    | "CanGoPrevious"
                                    | "CanPlay"
                                    | "CanPause"
                                    | "CanSeek"
                                    | "CanControl" => true.to_variant(),
                                    _ => false.to_variant(),
                                }
                            },
                        )
                        .set_property(
                            move |_conn, _sender, _path, _iface, prop_name, value| {
                                match prop_name {
                                    "Volume" => {
                                        if let Some(v) = value.get::<f64>() {
                                            cmd_set_prop(MprisCommand::SetVolume(
                                                (v * 100.0).clamp(0.0, 150.0),
                                            ));
                                        }
                                    }
                                    "Rate" => {
                                        if let Some(r) = value.get::<f64>() {
                                            cmd_set_prop(MprisCommand::SetRate(r.clamp(0.1, 8.0)));
                                        }
                                    }
                                    _ => {}
                                }
                                true
                            },
                        )
                        .build();
                }
            },
            |_conn, _name| {},
            |_conn, _name| {},
        );

        Self {
            state,
            _owner_id: Rc::new(owner_id),
        }
    }

    pub fn set_playback_status(&self, paused: bool, idle: bool) {
        let next = if idle {
            "Stopped"
        } else if paused {
            "Paused"
        } else {
            "Playing"
        };
        let mut st = self.state.borrow_mut();
        if st.playback_status != next {
            st.playback_status = next.to_string();
            let mut changed = HashMap::new();
            changed.insert("PlaybackStatus", st.playback_status.to_variant());
            emit_properties_changed(st.connection.as_ref(), changed);
        }
    }

    pub fn set_position(&self, seconds: f64) {
        let pos_us = (seconds.max(0.0) * 1_000_000.0) as i64;
        self.state.borrow_mut().position_us = pos_us;
    }

    pub fn emit_seeked(&self, seconds: f64) {
        let pos_us = (seconds.max(0.0) * 1_000_000.0) as i64;
        let mut st = self.state.borrow_mut();
        st.position_us = pos_us;
        if let Some(conn) = st.connection.as_ref() {
            let _ = conn.emit_signal(
                None,
                "/org/mpris/MediaPlayer2",
                "org.mpris.MediaPlayer2.Player",
                "Seeked",
                Some(&(&(pos_us,)).to_variant()),
            );
        }
    }

    pub fn set_metadata(&self, title: Option<&str>, url: Option<&str>, duration_sec: Option<f64>) {
        let mut st = self.state.borrow_mut();
        let mut dirty = false;
        if let Some(t) = title {
            if st.title != t {
                st.title = t.to_string();
                dirty = true;
            }
        }
        if let Some(u) = url {
            if st.url != u {
                st.url = u.to_string();
                dirty = true;
            }
        }
        if let Some(d) = duration_sec {
            let dur_us = (d.max(0.0) * 1_000_000.0) as i64;
            if st.duration_us != dur_us {
                st.duration_us = dur_us;
                dirty = true;
            }
        }
        if dirty {
            let meta = build_metadata_variant(&st);
            let mut changed = HashMap::new();
            changed.insert("Metadata", meta);
            emit_properties_changed(st.connection.as_ref(), changed);
        }
    }

    pub fn set_volume(&self, volume_pct: f64) {
        let vol = (volume_pct / 100.0).clamp(0.0, 1.5);
        let mut st = self.state.borrow_mut();
        if (st.volume - vol).abs() > 0.005 {
            st.volume = vol;
            let mut changed = HashMap::new();
            changed.insert("Volume", vol.to_variant());
            emit_properties_changed(st.connection.as_ref(), changed);
        }
    }

    pub fn set_rate(&self, rate: f64) {
        let mut st = self.state.borrow_mut();
        if (st.rate - rate).abs() > 0.005 {
            st.rate = rate;
            let mut changed = HashMap::new();
            changed.insert("Rate", rate.to_variant());
            emit_properties_changed(st.connection.as_ref(), changed);
        }
    }
}

fn build_metadata_variant(st: &MprisState) -> Variant {
    let mut map: HashMap<&str, Variant> = HashMap::new();
    if let Ok(track_id) = glib::variant::ObjectPath::try_from("/org/mpris/MediaPlayer2/Track/1") {
        map.insert("mpris:trackid", track_id.to_variant());
    }
    map.insert("mpris:length", st.duration_us.to_variant());
    map.insert("xesam:title", st.title.to_variant());
    if !st.url.is_empty() {
        map.insert("xesam:url", st.url.to_variant());
    }
    map.to_variant()
}

fn emit_properties_changed(
    conn: Option<&gio::DBusConnection>,
    changed: HashMap<&str, Variant>,
) {
    let Some(connection) = conn else {
        return;
    };
    let invalidated: Vec<&str> = Vec::new();
    let payload = (
        "org.mpris.MediaPlayer2.Player",
        changed,
        invalidated,
    )
        .to_variant();
    let _ = connection.emit_signal(
        None,
        "/org/mpris/MediaPlayer2",
        "org.freedesktop.DBus.Properties",
        "PropertiesChanged",
        Some(&payload),
    );
}
