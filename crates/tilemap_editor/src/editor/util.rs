use bevy::prelude::*;
use bevy::ecs::system::Command;

struct DespawnSilently(Entity);

impl Command for DespawnSilently {
	fn apply(self, world: &mut World) {
		if world.entities().contains(self.0) {
			let _ = world.despawn(self.0);
		}
	}
}

/// 安全地 despawn 一个实体：即使实体已不存在/被复用，也不会触发 bevy 的 command error。
pub fn despawn_silently(commands: &mut Commands, entity: Entity) {
	commands.queue(DespawnSilently(entity));
}
