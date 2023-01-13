use std::{
    time::Duration,
    path::PathBuf,
    sync::RwLock,
};
use serde_json as json;

use actix::{Actor, StreamHandler};
use actix_web::{web, Error, HttpRequest, HttpResponse, HttpServer, App};
use actix_web_actors::ws;
use actix_files::NamedFile;
use actix_rt;

use rapier3d;
use rapier3d::prelude::*;

// Static files

async fn index_route(_req: HttpRequest) -> Result<NamedFile, Error> {
    Ok(NamedFile::open("static/index.html")?)
}

async fn static_route(req: HttpRequest) -> Result<NamedFile, Error> {
    let mut path: PathBuf = "static".into();
    let fname: PathBuf = req.match_info().query("filename").parse().unwrap();
    path.push(fname);
    Ok(NamedFile::open(path)?)
}

/// Player Websocket

struct PlayerWs {
    app_state: web::Data<RwLock<AppState>>,
}

impl Actor for PlayerWs {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for PlayerWs {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        if let Ok(ws::Message::Text(text)) = msg {
            if let Some(idx) = text.find(':') {
                if let Some(key) = text.get(..idx) {
                    let res = match key {
                        "UPD" => self.handle_update_request(),
                        _ => None,
                    };
                    if let Some(res_text) = res {
                        ctx.text(res_text)
                    };
                }
            }
        }
    }
}

impl PlayerWs {
    fn handle_update_request(&self) -> Option<String> {
        let data_r = self.app_state.read().unwrap();
        Some(data_r.game_state.clone())
    }
}

async fn websocket_route(req: HttpRequest, stream: web::Payload, app_state: web::Data<RwLock<AppState>>) -> Result<HttpResponse, Error> {
    let resp = ws::start(PlayerWs {
        app_state: app_state
    }, &req, stream);
    // println!("{:?}", resp);
    resp
}

// Game

struct PhysicEngine {
    num_step: u64,
    physics_pipeline: PhysicsPipeline,
    gravity: Vector<Real>,
    integration_parameters: IntegrationParameters,
    island_manager: IslandManager,
    broad_phase: BroadPhase,
    narrow_phase: NarrowPhase,
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
}

impl PhysicEngine {

    fn create() -> PhysicEngine {
        PhysicEngine{
            num_step: 0,
            physics_pipeline: PhysicsPipeline::new(),
            gravity: vector![0.0, -9.81, 0.0],
            integration_parameters: IntegrationParameters::default(),
            island_manager: IslandManager::new(),
            broad_phase: BroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
        }
    }

    fn add_ground(&mut self) {
        self.collider_set.insert(
            ColliderBuilder::cuboid(100.0, 0.1, 100.0)
                .build()
        );
    }

    fn add_body(&mut self, pos: Vector<f32>) {
        let ball_body_handle = self.rigid_body_set.insert(
            RigidBodyBuilder::dynamic()
                .translation(pos)
                .build()
        );
        self.collider_set.insert_with_parent(
            ColliderBuilder::ball(0.5)
                .restitution(0.7)
                .build(),
            ball_body_handle,
            &mut self.rigid_body_set,
        );
    }

    fn step(&mut self) {
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            &(),
            &(),
        );
        self.num_step += 1;
    }

    fn export(&self) -> String {
        json::json!({
            "step": self.num_step,
            "time": (self.num_step as f32) * self.integration_parameters.dt,
            "bodies": self.rigid_body_set.iter().map(|rb| {
                let id = rb.0.into_raw_parts();
                let pos = rb.1.translation();
                json::json!({
                    "id": format!("{}-{}", id.0, id.1),
                    "pos": [pos.x, pos.y, pos.z],
                })
            }).collect::<json::Value>()
        }).to_string()
    }
}

// App

struct AppState {
    game_state: String,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {

    const PORT: u16 = 8080;

    let app_state = web::Data::new(RwLock::new(AppState {
        game_state: String::from("")
    }));

    let mut physic_engine = PhysicEngine::create();
    physic_engine.add_ground();
    physic_engine.add_body(vector![0.0, 10.0, 0.0]);
    physic_engine.add_body(vector![-5.0, 5.0, 0.0]);
    physic_engine.add_body(vector![5.0, 15.0, 0.0]);

    let app_state_w = app_state.clone();
    actix_rt::spawn(async move {
        let mut interval = actix_rt::time::interval(Duration::from_millis(
            (physic_engine.integration_parameters.dt * 1000.) as u64
        ));
        loop {
            interval.tick().await;
            physic_engine.step();
            let mut data_w = app_state_w.write().unwrap();
            data_w.game_state = physic_engine.export();
        }
    });

    println!("Start HTTP server: http://127.0.0.1:{}", PORT);
    HttpServer::new(move || App::new()
        .app_data(app_state.clone())
        .route("/", web::get().to(index_route))
        .route("/static/{filename:.*}", web::get().to(static_route))
        .route("/ws", web::get().to(websocket_route))
    )
    .bind(("127.0.0.1", PORT))?
    .run()
    .await
}