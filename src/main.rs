use ggez::GameResult;
use ggez::graphics::{self, Color, DrawParam, Canvas, Image, Mesh, MeshBuilder};
use ggez::event::{self, EventHandler};
use ggez::input::keyboard::{self, KeyCode};
use glam::Vec2;
use rand::Rng;
use std::time::Duration;
use std::collections::HashSet;
use std::path;
use std::env;
use ggez::audio;
use ggez::audio::{SoundSource, Source};
use rand::distributions::Uniform;

const WINDOW_WIDTH: f32 = 1024.0;
const WINDOW_HEIGHT: f32 = 768.0;
const PLAYER_SPEED: f32 = 5.0;
const BULLET_SPEED: f32 = 8.0;
const ENEMY_SPEED: f32 = 2.0;

// 优化粒子系统的常量
const PARTICLE_LIFETIME: f32 = 0.5; // 减少粒子生命周期
const EXPLOSION_PARTICLES: i32 = 10; // 减少每次爆炸的粒子数量
const PARTICLE_SPEED: f32 = 50.0; // 降低粒子速度
const PARTICLE_SIZE: f32 = 2.0;
const MAX_PARTICLES: usize = 1000; // 添加最大粒子数量限制

// 资源目录名称常量
const RESOURCE_DIR: &str = "resources";

enum GameObjectType {
    Player,
    Bullet,
    Enemy,
}

struct GameObject {
    pos: Vec2,
    size: Vec2,
    speed: Vec2,
    image: Option<Image>,
    rotation: f32,  // 添加旋转属性
    object_type: GameObjectType,
}




impl GameObject {
    fn new(ctx: &mut ggez::Context, x: f32, y: f32, width: f32, height: f32, object_type: GameObjectType) -> GameResult<Self> {
        let (image, rotation) = match object_type {
            GameObjectType::Player => (Some(Image::from_path(ctx, "/img/player.png")?), 0.0),
            GameObjectType::Bullet => (Some(Image::from_path(ctx, "/img/bullet.png")?), 0.0),
            GameObjectType::Enemy => (
                Some(Image::from_path(ctx, "/img/player.png")?),
                std::f32::consts::PI  // 敌人旋转180度
            ),
        };

        Ok(GameObject {
            pos: Vec2::new(x, y),
            size: Vec2::new(width, height),
            speed: Vec2::ZERO,
            image,
            rotation,
            object_type,
        })
    }

    fn draw(&self, canvas: &mut Canvas) {
        if let Some(ref image) = self.image {
            canvas.draw(
                image,
                DrawParam::default()
                    .dest(self.pos)
                    .rotation(self.rotation)  // 应用旋转
                    .offset(Vec2::new(0.5, 0.5))  // 设置旋转中心点为图片中心
                    .scale(Vec2::new(
                        self.size.x / image.width() as f32,
                        self.size.y / image.height() as f32
                    ))
            );
        }
    }

    // intersects 方法保持不变
    fn intersects(&self, other: &GameObject) -> bool {
        self.pos.x < other.pos.x + other.size.x &&
            self.pos.x + self.size.x > other.pos.x &&
            self.pos.y < other.pos.y + other.size.y &&
            self.pos.y + self.size.y > other.pos.y
    }
}

#[derive(Clone)] // 添加 Clone trait
struct Particle {
    pos: Vec2,
    vel: Vec2,
    color: Color,
    lifetime: f32,
    size: f32,
}

impl Particle {
    fn new(pos: Vec2, vel: Vec2, color: Color, size: f32) -> Self {
        Particle {
            pos,
            vel,
            color,
            lifetime: PARTICLE_LIFETIME,
            size,
        }
    }

    fn update(&mut self, dt: f32) {
        self.pos += self.vel * dt;
        self.lifetime -= dt;
        self.color.a = (self.lifetime / PARTICLE_LIFETIME).min(1.0);
        self.size = self.size * (self.lifetime / PARTICLE_LIFETIME).max(0.1);
    }
}

// 创建专门的粒子系统结构体
struct ParticleSystem {
    particles: Vec<Particle>,
}

impl ParticleSystem {
    fn new() -> Self {
        ParticleSystem {
            particles: Vec::with_capacity(MAX_PARTICLES), // 预分配内存
        }
    }

    fn update(&mut self, dt: f32) {
        self.particles.retain_mut(|particle| {
            particle.update(dt);
            particle.lifetime > 0.0
        });
    }

    fn add_explosion(&mut self, pos: Vec2, color: Color) {
        let mut rng = rand::thread_rng();

        // 确保不超过最大粒子数量
        let available_slots = MAX_PARTICLES.saturating_sub(self.particles.len());
        let particles_to_add = EXPLOSION_PARTICLES.min(available_slots as i32);

        for _ in 0..particles_to_add {
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            let speed = rng.gen_range(PARTICLE_SPEED * 0.5..PARTICLE_SPEED);
            let vel = Vec2::new(angle.cos() * speed, angle.sin() * speed);
            let size = rng.gen_range(PARTICLE_SIZE * 0.5..PARTICLE_SIZE * 1.5);

            let particle = Particle::new(pos, vel, color, size);
            self.particles.push(particle);
        }
    }

    fn draw(&self, ctx: &mut ggez::Context, canvas: &mut Canvas) -> GameResult {
        for particle in &self.particles {
            let mesh = Mesh::new_circle(
                ctx,
                graphics::DrawMode::fill(),
                particle.pos,
                particle.size,
                0.1,
                particle.color,
            )?;
            canvas.draw(&mesh, DrawParam::default());
        }
        Ok(())
    }
}


// 添加声音资源结构体
struct SoundEffects {
    shoot_sound: Source,
    explosion_sound: Source,
}
impl SoundEffects {
    fn new(ctx: &mut ggez::Context) -> GameResult<Self> {
        let shoot_sound = Source::new(ctx, "/sound/shoot.wav")?;
        let explosion_sound = Source::new(ctx, "/sound/expl1.wav")?;

        Ok(SoundEffects {
            shoot_sound,
            explosion_sound,
        })
    }

    fn play_shoot(&mut self, ctx: &mut ggez::Context) -> GameResult {
        if self.shoot_sound.playing() {
            self.shoot_sound.stop(ctx)?;
        }
        self.shoot_sound.play(ctx)?;
        Ok(())
    }

    fn play_explosion(&mut self, ctx: &mut ggez::Context) -> GameResult {
        if self.explosion_sound.playing() {
            self.explosion_sound.stop(ctx)?;
        }
        self.explosion_sound.play(ctx)?;
        Ok(())
    }
}



struct MainState {
    player: GameObject,
    bullets: Vec<GameObject>,
    enemies: Vec<GameObject>,
    score: i32,
    spawn_timer: Duration,
    game_over: bool,
    shoot_cooldown: Duration,
    star_field: Vec<(Vec2, f32)>,
    particles: ParticleSystem,  // 修改字段名以匹配初始化
    sounds: SoundEffects,  // 添加声音系统
}

impl MainState {

    fn spawn_enemy(&mut self, ctx: &mut ggez::Context) -> GameResult {
        let mut rng = rand::thread_rng();
        let x = rng.gen_range(0.0..WINDOW_WIDTH - 40.0);
        let enemy = GameObject::new(
            ctx,
            x,
            -50.0,  // 在屏幕上方生成
            40.0,   // 敌人宽度
            40.0,   // 敌人高度
            GameObjectType::Enemy,
        )?;
        self.enemies.push(enemy);
        Ok(())
    }

    fn new(ctx: &mut ggez::Context) -> GameResult<MainState> {
        let player = GameObject::new(
            ctx,
            WINDOW_WIDTH / 2.0 - 25.0,
            WINDOW_HEIGHT - 60.0,
            50.0,
            60.0,
            GameObjectType::Player,
        )?;

        let mut star_field = Vec::new();
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            star_field.push((
                Vec2::new(
                    rng.gen_range(0.0..WINDOW_WIDTH),
                    rng.gen_range(0.0..WINDOW_HEIGHT)
                ),
                rng.gen_range(1.0..3.0)
            ));
        }

        // 初始化声音系统
        let mut sounds = SoundEffects::new(ctx)?;
        sounds.shoot_sound.set_volume(0.3);
        sounds.explosion_sound.set_volume(0.5);


        Ok(MainState {
            player,
            bullets: Vec::new(),
            enemies: Vec::new(),
            score: 0,
            spawn_timer: Duration::from_secs(0),
            game_over: false,
            shoot_cooldown: Duration::from_secs(0),
            star_field,
            particles: ParticleSystem::new(),
            sounds,
        })
    }

    fn shoot(&mut self, ctx: &mut ggez::Context) -> GameResult {

        // 播放射击音效
        self.sounds.play_shoot(ctx)?;

        let bullet_pos = Vec2::new(
            self.player.pos.x + self.player.size.x / 2.0 - 2.5,
            self.player.pos.y,
        );

        self.particles.add_explosion(
            bullet_pos,
            Color::new(1.0, 1.0, 0.0, 0.5),
        );

        let bullet = GameObject::new(
            ctx,
            bullet_pos.x,
            bullet_pos.y,
            5.0,
            20.0,
            GameObjectType::Bullet,
        )?;
        self.bullets.push(bullet);
        Ok(())
    }
}

impl EventHandler for MainState {
    fn update(&mut self, ctx: &mut ggez::Context) -> GameResult {
        if self.game_over {
            return Ok(());
        }

        let mut dx = 0.0;
        let mut dy = 0.0;

        if keyboard::is_key_pressed(ctx, KeyCode::Left) || keyboard::is_key_pressed(ctx, KeyCode::A) {
            dx -= PLAYER_SPEED;
        }
        if keyboard::is_key_pressed(ctx, KeyCode::Right) || keyboard::is_key_pressed(ctx, KeyCode::D) {
            dx += PLAYER_SPEED;
        }
        if keyboard::is_key_pressed(ctx, KeyCode::Up) || keyboard::is_key_pressed(ctx, KeyCode::W) {
            dy -= PLAYER_SPEED;
        }
        if keyboard::is_key_pressed(ctx, KeyCode::Down) || keyboard::is_key_pressed(ctx, KeyCode::S) {
            dy += PLAYER_SPEED;
        }

        self.player.pos.x += dx;
        self.player.pos.y += dy;

        self.player.pos.x = self.player.pos.x.clamp(0.0, WINDOW_WIDTH - self.player.size.x);
        self.player.pos.y = self.player.pos.y.clamp(0.0, WINDOW_HEIGHT - self.player.size.y);

        self.shoot_cooldown = self.shoot_cooldown.saturating_sub(ctx.time.delta());

        if keyboard::is_key_pressed(ctx, KeyCode::Space) && self.shoot_cooldown.is_zero() {
            self.shoot(ctx)?;
            self.shoot_cooldown = Duration::from_millis(250);
        }

        for bullet in &mut self.bullets {
            bullet.pos.y -= BULLET_SPEED;
        }
        self.bullets.retain(|bullet| bullet.pos.y > -bullet.size.y);

        self.spawn_timer += ctx.time.delta();
        if self.spawn_timer.as_secs_f32() >= 1.0 {
            self.spawn_enemy(ctx)?;
            self.spawn_timer = Duration::from_secs(0);
        }

        for enemy in &mut self.enemies {
            enemy.pos.y += ENEMY_SPEED;
            if enemy.intersects(&self.player) {
                self.game_over = true;
            }
        }
        self.enemies.retain(|enemy| enemy.pos.y < WINDOW_HEIGHT);

        for (pos, _) in &mut self.star_field {
            pos.y += 0.5;
            if pos.y > WINDOW_HEIGHT {
                pos.y = 0.0;
            }
        }




        // 修改碰撞检测部分，收集爆炸位置
        let mut destroyed_bullets = HashSet::new();
        let mut destroyed_enemies = HashSet::new();
        // 收集爆炸位置
        let mut explosion_positions = Vec::new();

        for (bullet_idx, bullet) in self.bullets.iter().enumerate() {
            for (enemy_idx, enemy) in self.enemies.iter().enumerate() {
                if !destroyed_bullets.contains(&bullet_idx) &&
                    !destroyed_enemies.contains(&enemy_idx) &&
                    bullet.intersects(enemy) {
                    destroyed_bullets.insert(bullet_idx);
                    destroyed_enemies.insert(enemy_idx);
                    self.score += 10;

                    // 播放爆炸音效
                    self.sounds.play_explosion(ctx)?;

                    explosion_positions.push((
                        Vec2::new(
                            enemy.pos.x + enemy.size.x / 2.0,
                            enemy.pos.y + enemy.size.y / 2.0
                        ),
                        Color::new(1.0, 0.5, 0.0, 1.0)
                    ));
                }
            }
        }

        // 移除被销毁的对象
        let mut bullets_to_remove: Vec<_> = destroyed_bullets.into_iter().collect();
        let mut enemies_to_remove: Vec<_> = destroyed_enemies.into_iter().collect();
        bullets_to_remove.sort_unstable_by(|a, b| b.cmp(a));
        enemies_to_remove.sort_unstable_by(|a, b| b.cmp(a));

        for idx in bullets_to_remove {
            if idx < self.bullets.len() {
                self.bullets.remove(idx);
            }
        }
        for idx in enemies_to_remove {
            if idx < self.enemies.len() {
                self.enemies.remove(idx);
            }
        }

        // 在碰撞检测后创建爆炸效果时使用正确的字段名
        for (pos, color) in explosion_positions {
            self.particles.add_explosion(pos, color);
        }

        // 更新粒子系统时使用正确的字段名
        self.particles.update(ctx.time.delta().as_secs_f32());

        Ok(())
    }


    fn draw(&mut self, ctx: &mut ggez::Context) -> GameResult {
        let mut canvas = Canvas::from_frame(ctx, Color::new(0.0, 0.05, 0.1, 1.0));

        // 绘制星星
        for (pos, size) in &self.star_field {
            let star = Mesh::new_circle(
                ctx,
                graphics::DrawMode::fill(),
                *pos,
                *size,
                0.1,
                Color::WHITE,
            )?;
            canvas.draw(&star, DrawParam::default());
        }

        // 绘制游戏对象
        self.player.draw(&mut canvas);

        for bullet in &self.bullets {
            bullet.draw(&mut canvas);
        }

        for enemy in &self.enemies {
            enemy.draw(&mut canvas);
        }

        // 使用正确的字段名绘制粒子
        self.particles.draw(ctx, &mut canvas)?;

        // 绘制 UI
        let score_text = graphics::Text::new(format!("Score: {}", self.score));
        canvas.draw(
            &score_text,
            DrawParam::default()
                .dest(Vec2::new(10.0, 10.0))
                .color(Color::WHITE)
        );

        if self.game_over {
            let game_over_text = graphics::Text::new("Game Over!");
            let params = DrawParam::default()
                .dest(Vec2::new(WINDOW_WIDTH/2.0 - 100.0, WINDOW_HEIGHT/2.0))
                .color(Color::RED);
            canvas.draw(&game_over_text, params);
        }

        canvas.finish(ctx)?;
        Ok(())
    }

}

fn main() -> GameResult {
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let mut path = path::PathBuf::from(manifest_dir);
        path.push(RESOURCE_DIR);
        println!("Adding resource path: {:?}", path);
        env::set_var("CARGO_RESOURCE_ROOT", path);
    }

    let cb = ggez::ContextBuilder::new("vertical_shooter", "author")
        .window_setup(ggez::conf::WindowSetup::default()
            .title("Vertical Shooter"))
        .window_mode(ggez::conf::WindowMode::default()
            .dimensions(WINDOW_WIDTH, WINDOW_HEIGHT))
        .add_resource_path(path::PathBuf::from(RESOURCE_DIR));

    let (mut ctx, event_loop) = cb.build()?;
    let state = MainState::new(&mut ctx)?;
    event::run(ctx, event_loop, state)
}