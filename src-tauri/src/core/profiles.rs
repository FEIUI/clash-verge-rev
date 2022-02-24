use super::{Clash, ClashInfo};
use crate::utils::{config, dirs, tmpl};
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};
use std::collections::HashMap;
use std::env::temp_dir;
use std::fs::{remove_file, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Define the `profiles.yaml` schema
#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct Profiles {
  /// current profile's name
  pub current: Option<usize>,

  /// profile list
  pub items: Option<Vec<ProfileItem>>,
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct ProfileItem {
  /// profile name
  pub name: Option<String>,
  /// profile description
  pub desc: Option<String>,
  /// profile file
  pub file: Option<String>,
  /// current mode
  pub mode: Option<String>,
  /// source url
  pub url: Option<String>,
  /// selected infomation
  pub selected: Option<Vec<ProfileSelected>>,
  /// user info
  pub extra: Option<ProfileExtra>,
  /// updated time
  pub updated: Option<usize>,
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct ProfileSelected {
  pub name: Option<String>,
  pub now: Option<String>,
}

#[derive(Default, Debug, Clone, Copy, Deserialize, Serialize)]
pub struct ProfileExtra {
  pub upload: usize,
  pub download: usize,
  pub total: usize,
  pub expire: usize,
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
/// the result from url
pub struct ProfileResponse {
  pub name: String,
  pub file: String,
  pub data: String,
  pub extra: Option<ProfileExtra>,
}

static PROFILE_YAML: &str = "profiles.yaml";
static PROFILE_TEMP: &str = "clash-verge-runtime.yaml";

impl Profiles {
  /// read the config from the file
  pub fn read_file() -> Self {
    config::read_yaml::<Profiles>(dirs::app_home_dir().join(PROFILE_YAML))
  }

  /// save the config to the file
  pub fn save_file(&self) -> Result<(), String> {
    config::save_yaml(
      dirs::app_home_dir().join(PROFILE_YAML),
      self,
      Some("# Profiles Config for Clash Verge\n\n"),
    )
  }

  /// sync the config between file and memory
  pub fn sync_file(&mut self) -> Result<(), String> {
    let data = config::read_yaml::<Self>(dirs::app_home_dir().join(PROFILE_YAML));
    if data.current.is_none() {
      Err("failed to read profiles.yaml".into())
    } else {
      self.current = data.current;
      self.items = data.items;
      Ok(())
    }
  }

  /// import the new profile from the url
  /// and update the config file
  pub fn import_from_url(&mut self, url: String, result: ProfileResponse) -> Result<(), String> {
    // save the profile file
    let path = dirs::app_home_dir().join("profiles").join(&result.file);
    let file_data = result.data.as_bytes();
    File::create(path).unwrap().write(file_data).unwrap();

    // update `profiles.yaml`
    let data = Profiles::read_file();
    let mut items = data.items.unwrap_or(vec![]);

    let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_secs();

    items.push(ProfileItem {
      name: Some(result.name),
      desc: Some("imported url".into()),
      file: Some(result.file),
      mode: Some(format!("rule")),
      url: Some(url),
      selected: Some(vec![]),
      extra: result.extra,
      updated: Some(now as usize),
    });

    self.items = Some(items);
    if data.current.is_none() {
      self.current = Some(0);
    }

    self.save_file()
  }

  /// set the current and save to file
  pub fn put_current(&mut self, index: usize) -> Result<(), String> {
    let items = self.items.take().unwrap_or(vec![]);

    if index >= items.len() {
      return Err("the index out of bound".into());
    }

    self.items = Some(items);
    self.current = Some(index);
    self.save_file()
  }

  /// append new item
  /// return the new item's index
  pub fn append_item(&mut self, name: String, desc: String) -> Result<(usize, PathBuf), String> {
    let mut items = self.items.take().unwrap_or(vec![]);

    // create a new profile file
    let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_secs();
    let file = format!("{}.yaml", now);
    let path = dirs::app_home_dir().join("profiles").join(&file);

    match File::create(&path).unwrap().write(tmpl::ITEM_CONFIG) {
      Ok(_) => {
        items.push(ProfileItem {
          name: Some(name),
          desc: Some(desc),
          file: Some(file),
          mode: None,
          url: None,
          selected: Some(vec![]),
          extra: None,
          updated: Some(now as usize),
        });

        let index = items.len();
        self.items = Some(items);
        Ok((index, path))
      }
      Err(_) => Err("failed to create file".into()),
    }
  }

  /// update the target profile
  /// and save to config file
  /// only support the url item
  pub fn update_item(&mut self, index: usize, result: ProfileResponse) -> Result<(), String> {
    let mut items = self.items.take().unwrap_or(vec![]);

    let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_secs() as usize;

    // update file
    let file_path = &items[index].file.as_ref().unwrap();
    let file_path = dirs::app_home_dir().join("profiles").join(file_path);
    let file_data = result.data.as_bytes();
    File::create(file_path).unwrap().write(file_data).unwrap();

    items[index].name = Some(result.name);
    items[index].extra = result.extra;
    items[index].updated = Some(now);

    self.items = Some(items);
    self.save_file()
  }

  /// patch item
  pub fn patch_item(&mut self, index: usize, profile: ProfileItem) -> Result<(), String> {
    let mut items = self.items.take().unwrap_or(vec![]);
    if index >= items.len() {
      return Err("index out of bound".into());
    }

    if profile.name.is_some() {
      items[index].name = profile.name;
    }
    if profile.file.is_some() {
      items[index].file = profile.file;
    }
    if profile.mode.is_some() {
      items[index].mode = profile.mode;
    }
    if profile.url.is_some() {
      items[index].url = profile.url;
    }
    if profile.selected.is_some() {
      items[index].selected = profile.selected;
    }
    if profile.extra.is_some() {
      items[index].extra = profile.extra;
    }

    self.items = Some(items);
    self.save_file()
  }

  /// delete the item
  pub fn delete_item(&mut self, index: usize) -> Result<bool, String> {
    let mut current = self.current.clone().unwrap_or(0);
    let mut items = self.items.clone().unwrap_or(vec![]);

    if index >= items.len() {
      return Err("index out of bound".into());
    }

    let mut rm_item = items.remove(index);

    // delete the file
    if let Some(file) = rm_item.file.take() {
      let file_path = dirs::app_home_dir().join("profiles").join(file);

      if file_path.exists() {
        if let Err(err) = remove_file(file_path) {
          log::error!("{err}");
        }
      }
    }

    let mut should_change = false;

    if current == index {
      current = 0;
      should_change = true;
    } else if current > index {
      current = current - 1;
    }

    self.current = Some(current);
    self.items = Some(items);

    match self.save_file() {
      Ok(_) => Ok(should_change),
      Err(err) => Err(err),
    }
  }

  /// activate current profile
  pub fn activate(&self, clash: &Clash) -> Result<(), String> {
    let current = self.current.unwrap_or(0);
    match self.items.clone() {
      Some(items) => {
        if current >= items.len() {
          return Err("the index out of bound".into());
        }

        let profile = items[current].clone();
        let clash_config = clash.config.clone();
        let clash_info = clash.info.clone();
        tauri::async_runtime::spawn(async move {
          let mut count = 5; // retry times
          let mut err = String::from("");
          while count > 0 {
            match activate_profile(&profile, &clash_config, &clash_info).await {
              Ok(_) => return,
              Err(e) => err = e,
            }
            count -= 1;
          }
          log::error!("failed to activate for `{}`", err);
        });

        Ok(())
      }
      None => Err("empty profiles".into()),
    }
  }
}

/// put the profile to clash
pub async fn activate_profile(
  profile_item: &ProfileItem,
  clash_config: &Mapping,
  clash_info: &ClashInfo,
) -> Result<(), String> {
  // temp profile's path
  let temp_path = temp_dir().join(PROFILE_TEMP);

  // generate temp profile
  {
    let file_name = match profile_item.file.clone() {
      Some(file_name) => file_name,
      None => return Err("profile item should have `file` field".into()),
    };

    let file_path = dirs::app_home_dir().join("profiles").join(file_name);
    if !file_path.exists() {
      return Err(format!(
        "profile `{}` not exists",
        file_path.as_os_str().to_str().unwrap()
      ));
    }

    // begin to generate the new profile config
    let def_config = config::read_yaml::<Mapping>(file_path.clone());

    // use the clash config except 5 keys below
    let mut new_config = clash_config.clone();

    // Only the following fields are allowed:
    // proxies/proxy-providers/proxy-groups/rule-providers/rules
    let valid_keys = vec![
      "proxies",
      "proxy-providers",
      "proxy-groups",
      "rule-providers",
      "rules",
    ];
    valid_keys.iter().for_each(|key| {
      let key = Value::String(key.to_string());
      if def_config.contains_key(&key) {
        let value = def_config[&key].clone();
        new_config.insert(key, value);
      }
    });

    config::save_yaml(
      temp_path.clone(),
      &new_config,
      Some("# Clash Verge Temp File"),
    )?
  };

  let server = format!("http://{}/configs", clash_info.server.clone().unwrap());

  let mut headers = HeaderMap::new();
  headers.insert("Content-Type", "application/json".parse().unwrap());

  if let Some(secret) = clash_info.secret.clone() {
    headers.insert(
      "Authorization",
      format!("Bearer {}", secret).parse().unwrap(),
    );
  }

  let mut data = HashMap::new();
  data.insert("path", temp_path.as_os_str().to_str().unwrap());

  let client = match reqwest::ClientBuilder::new().no_proxy().build() {
    Ok(c) => c,
    Err(_) => return Err("failed to create http::put".into()),
  };
  match client.put(server).headers(headers).json(&data).send().await {
    Ok(_) => Ok(()),
    Err(err) => Err(format!("request failed `{}`", err.to_string())),
  }
}