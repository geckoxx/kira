use basedrop::{Handle, Shared};
use ringbuf::Producer;

use crate::{
	manager::command::Command,
	sound::instance::{Instance, InstanceController},
	tempo::Tempo,
};

use super::{RawSequence, SequenceStep};

/// The playback state of an instance of a sequence.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SequenceInstanceState {
	/// The sequence instance is playing.
	Playing,
	/// The sequence instance is paused.
	///
	/// Any wait commands are currently on hold.
	Paused,
	/// The sequence has finished and will not perform
	/// any more actions.
	Finished,
}

pub struct SequenceInstance {
	sequence: RawSequence,
	instance_controllers: Vec<Shared<InstanceController>>,
	state: SequenceInstanceState,
	position: usize,
	wait_timer: Option<f64>,
	instance_queue: Vec<Instance>,
	event_producer: Producer<usize>,
}

impl SequenceInstance {
	pub(crate) fn new(
		sequence: RawSequence,
		collector_handle: &Handle,
		event_producer: Producer<usize>,
	) -> Self {
		let num_instances = sequence.num_instances();
		let instance_controllers = {
			let mut instance_controllers = vec![];
			for _ in 0..num_instances {
				instance_controllers.push(Shared::new(collector_handle, InstanceController::new()));
			}
			instance_controllers
		};
		Self {
			sequence,
			instance_controllers,
			state: SequenceInstanceState::Playing,
			position: 0,
			wait_timer: None,
			instance_queue: Vec::with_capacity(num_instances),
			event_producer,
		}
	}

	fn set_state(&mut self, state: SequenceInstanceState) {
		self.state = state;
	}

	fn start_step(&mut self, index: usize) {
		if let Some(step) = self.sequence.steps.get(index) {
			self.position = index;
			if let SequenceStep::Wait(_) = step {
				self.wait_timer = Some(1.0);
			} else {
				self.wait_timer = None;
			}
		} else if let Some(loop_point) = self.sequence.loop_point {
			self.start_step(loop_point);
		} else {
			self.set_state(SequenceInstanceState::Finished);
		}
	}

	pub(crate) fn start(&mut self) {
		self.start_step(0);
	}

	pub(crate) fn pause(&mut self) {
		self.set_state(SequenceInstanceState::Paused);
	}

	pub(crate) fn resume(&mut self) {
		self.set_state(SequenceInstanceState::Playing);
	}

	pub(crate) fn stop(&mut self) {
		self.set_state(SequenceInstanceState::Finished);
	}

	pub(crate) fn update(&mut self, dt: f64, command_queue: &mut Vec<Command>) {
		match self.state {
			SequenceInstanceState::Paused | SequenceInstanceState::Finished => {
				return;
			}
			_ => {}
		}
		loop {
			if let Some(step) = self.sequence.steps.get(self.position) {
				match step {
					SequenceStep::Wait(duration) => {
						if let Some(time) = self.wait_timer.as_mut() {
							let duration = duration.in_seconds(Tempo(0.0));
							*time -= dt / duration;
							if *time <= 0.0 {
								self.start_step(self.position + 1);
							}
							break;
						}
					}
					SequenceStep::PlaySound(instance_id, sound) => {
						let controller = self.instance_controllers[instance_id.0].clone();
						controller.reset();
						let instance = Instance::new(sound.clone(), controller);
						command_queue.push(Command::StartInstance { instance });
						self.start_step(self.position + 1);
					}
					_ => {
						todo!()
					}
				}
			}
		}
	}

	pub(crate) fn finished(&self) -> bool {
		if let SequenceInstanceState::Finished = self.state {
			true
		} else {
			false
		}
	}
}
