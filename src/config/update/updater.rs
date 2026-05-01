use super::*;

pub fn update_update_channel(channel: UpdateChannel) {
    {
        let mut cfg = lock_config();
        if cfg.update_channel == channel {
            return;
        }
        cfg.update_channel = channel;
    }
    save_without_keymaps();
}
