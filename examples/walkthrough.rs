use nano::{
    access,
    events::EventQueue,
    globals::{Globals, IntoSingletonKey, Singleton},
    systems::{GlobalAccess, Scheduler},
};

#[derive(PartialEq, Eq, Hash)]
pub struct StartEvent;

#[derive(PartialEq, Eq, Hash)]
pub struct SayHelloEvent;

pub struct User {
    name: String,
}
pub struct Language {
    hello_msg: String,
}

fn main() {
    let mut scheduler = Scheduler::new();
    let mut globals = Globals::new();

    globals.insert(Singleton(Language {
        hello_msg: "Hello {} !".to_string(),
    }));

    scheduler.on(StartEvent, on_start);
    scheduler.on(SayHelloEvent, on_say_hello);

    scheduler.run(StartEvent, globals);
}

fn on_start(g: GlobalAccess) {
    access! { g |
        &mut event_queue: EventQueue::SINGLETON,
        &mut commands: Globals::COMMANDS,
    };

    commands.insert(Singleton(User {
        name: "Johny".to_string(),
    }));

    event_queue.push(SayHelloEvent);
}

fn on_say_hello(g: GlobalAccess) {
    access! { g |
        &user: User::SINGLETON,
        &language: Language::SINGLETON
    };

    println!("{}", language.hello_msg.replace("{}", &user.name));
}
