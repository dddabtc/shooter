// main.rs
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
use ggez::conf;

// 基准窗口尺寸
const BASE_WINDOW_WIDTH: f32 = 1024.0;
const BASE_WINDOW_HEIGHT: f32 = 768.0;

// 游戏常量现在使用相对值
const PLAYER_SPEED_RATIO: f32 = 5.0 / 1024.0; // 相对于窗口宽度的速度
const BULLET_SPEED_RATIO: f32 = 8.0 / 768.0;  // 相对于窗口高度的速度
const ENEMY_SPEED_RATIO: f32 = 2.0 / 768.0;   // 相对于窗口高度的速度

// 粒子系统常量
const PARTICLE_LIFETIME: f32 = 0.5;
const EXPLOSION_PARTICLES: i32 = 10;
const PARTICLE_SPEED: f32 = 50.0;
const PARTICLE_SIZE: f32 = 2.0;
const MAX_PARTICLES: usize = 1000;
const RESOURCE_DIR: &str = "resources";

// 窗口尺寸管理结构体
struct WindowSize {
    width: f32,
    height: f32,
    scale_x: f32,
    scale_y: f32,
}

impl WindowSize {
    fn new(width: f32, height: f32) -> Self {
        WindowSize {
            width,
            height,
            scale_x: width / BASE_WINDOW_WIDTH,
            scale_y: height / BASE_WINDOW_HEIGHT,
        }
    }

    fn scale_vec2(&self, vec: Vec2) -> Vec2 {
        Vec2::new(vec.x * self.scale_x, vec.y * self.scale_y)
    }

    fn unscale_vec2(&self, vec: Vec2) -> Vec2 {
        Vec2::new(vec.x / self.scale_x, vec.y / self.scale_y)
    }
}

// 游戏对象类型枚举
#[derive(Clone)]
enum GameObjectType {
    Player,
    Bullet,
    Enemy,
}

// 游戏对象结构体
struct GameObject {
    pos: Vec2,
    base_size: Vec2,
    speed: Vec2,
    image: Option<Image>,
    rotation: f32,
    object_type: GameObjectType,
}

impl GameObject {
    fn new(ctx: &mut ggez::Context, x: f32, y: f32, width: f32, height: f32, object_type: GameObjectType) -> GameResult<Self> {
        let (image, rotation) = match object_type {
            GameObjectType::Player => (Some(Image::from_path(ctx, "/img/player.png")?), 0.0),
            GameObjectType::Bullet => (Some(Image::from_path(ctx, "/img/bullet.png")?), 0.0),
            GameObjectType::Enemy => (Some(Image::from_path(ctx, "/img/player.png")?), std::f32::consts::PI),
        };

        Ok(GameObject {
            pos: Vec2::new(x, y),
            base_size: Vec2::new(width, height),
            speed: Vec2::ZERO,
            image,
            rotation,
            object_type,
        })
    }

    fn draw(&self, canvas: &mut Canvas, window_size: &WindowSize) {
        if let Some(ref image) = self.image {
            let scaled_pos = window_size.scale_vec2(self.pos);
            let scaled_size = window_size.scale_vec2(self.base_size);

            canvas.draw(
                image,
                DrawParam::default()
                    .dest(scaled_pos)
                    .rotation(self.rotation)
                    .offset(Vec2::new(0.5, 0.5))
                    .scale(Vec2::new(
                        scaled_size.x / image.width() as f32,
                        scaled_size.y / image.height() as f32
                    ))
            );
        }
    }

    fn intersects(&self, other: &GameObject, window_size: &WindowSize) -> bool {
        let scaled_pos = window_size.scale_vec2(self.pos);
        let scaled_size = window_size.scale_vec2(self.base_size);
        let other_scaled_pos = window_size.scale_vec2(other.pos);
        let other_scaled_size = window_size.scale_vec2(other.base_size);

        scaled_pos.x < other_scaled_pos.x + other_scaled_size.x &&
            scaled_pos.x + scaled_size.x > other_scaled_pos.x &&
            scaled_pos.y < other_scaled_pos.y + other_scaled_size.y &&
            scaled_pos.y + scaled_size.y > other_scaled_pos.y
    }
}

// 粒子结构体
#[derive(Clone)]
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

    fn update(&mut self, dt: f32, window_size: &WindowSize) {
        let scaled_vel = window_size.scale_vec2(self.vel);
        self.pos += scaled_vel * dt;
        self.lifetime -= dt;
        self.color.a = (self.lifetime / PARTICLE_LIFETIME).min(1.0);
        self.size = self.size * (self.lifetime / PARTICLE_LIFETIME).max(0.1);
    }
}

// 粒子系统结构体
struct ParticleSystem {
    particles: Vec<Particle>,
}

impl ParticleSystem {
    fn new() -> Self {
        ParticleSystem {
            particles: Vec::with_capacity(MAX_PARTICLES),
        }
    }

    fn update(&mut self, dt: f32, window_size: &WindowSize) {
        self.particles.retain_mut(|particle| {
            particle.update(dt, window_size);
            particle.lifetime > 0.0
        });
    }

    fn add_explosion(&mut self, pos: Vec2, color: Color, window_size: &WindowSize) {
        let mut rng = rand::thread_rng();
        let available_slots = MAX_PARTICLES.saturating_sub(self.particles.len());
        let particles_to_add = EXPLOSION_PARTICLES.min(available_slots as i32);

        for _ in 0..particles_to_add {
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            let base_speed  = PARTICLE_SPEED * window_size.scale_x.min(window_size.scale_y);
            let speed = rng.gen_range(base_speed * 0.5..base_speed);
            let vel = Vec2::new(angle.cos() * speed, angle.sin() * speed);
            let size = PARTICLE_SIZE * window_size.scale_x.min(window_size.scale_y);
            let scaled_size = rng.gen_range(size * 0.5..size * 1.5);

            self.particles.push(Particle::new(pos, vel, color, scaled_size));
        }
    }

    fn draw(&self, ctx: &mut ggez::Context, canvas: &mut Canvas, window_size: &WindowSize) -> GameResult {
        for particle in &self.particles {
            let scaled_pos = window_size.scale_vec2(particle.pos);
            let scaled_size = particle.size * window_size.scale_x.min(window_size.scale_y);

            // 创建一个简单的矩形代替圆形
            let rect = graphics::Rect::new(
                scaled_pos.x - scaled_size/2.0,
                scaled_pos.y - scaled_size/2.0,
                scaled_size,
                scaled_size,
            );

            let draw_param = DrawParam::default()
                .color(particle.color);

            // 直接绘制矩形
            canvas.draw(&graphics::Quad, draw_param.dest(rect.point()).scale([rect.w, rect.h]));
        }
        Ok(())
    }
}

// 声音系统结构体
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

// 主游戏状态结构体
struct MainState {
    window_size: WindowSize,
    player: GameObject,
    bullets: Vec<GameObject>,
    enemies: Vec<GameObject>,
    score: i32,
    spawn_timer: Duration,
    game_over: bool,
    shoot_cooldown: Duration,
    star_field: Vec<(Vec2, f32)>,
    particles: ParticleSystem,
    sounds: SoundEffects,
}

impl MainState {
    fn new(ctx: &mut ggez::Context) -> GameResult<MainState> {
        let window_size = WindowSize::new(BASE_WINDOW_WIDTH, BASE_WINDOW_HEIGHT);

        let player = GameObject::new(
            ctx,
            BASE_WINDOW_WIDTH / 2.0 - 25.0,
            BASE_WINDOW_HEIGHT - 60.0,
            50.0,
            60.0,
            GameObjectType::Player,
        )?;

        let mut star_field = Vec::new();
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            star_field.push((
                Vec2::new(
                    rng.gen_range(0.0..BASE_WINDOW_WIDTH),
                    rng.gen_range(0.0..BASE_WINDOW_HEIGHT)
                ),
                rng.gen_range(1.0..3.0)
            ));
        }

        let mut sounds = SoundEffects::new(ctx)?;
        sounds.shoot_sound.set_volume(0.3);
        sounds.explosion_sound.set_volume(0.5);

        Ok(MainState {
            window_size,
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

    fn spawn_enemy(&mut self, ctx: &mut ggez::Context) -> GameResult {
        let mut rng = rand::thread_rng();
        let x = rng.gen_range(0.0..BASE_WINDOW_WIDTH - 40.0);
        let enemy = GameObject::new(
            ctx,
            x,
            -50.0,
            40.0,
            40.0,
            GameObjectType::Enemy,
        )?;
        self.enemies.push(enemy);
        Ok(())
    }

    fn shoot(&mut self, ctx: &mut ggez::Context) -> GameResult {
        self.sounds.play_shoot(ctx)?;

        // 计算子弹发射位置：从飞机顶部中心发射
        let bullet_pos = Vec2::new(
            self.player.pos.x + (self.player.base_size.x / 2.0),  // 水平居中
            self.player.pos.y,  // 从飞机顶部发射
        );

        self.particles.add_explosion(
            bullet_pos,
            Color::new(1.0, 1.0, 0.0, 0.5),
            &self.window_size,
        );

        let bullet = GameObject::new(
            ctx,
            bullet_pos.x - 2.5,  // 考虑子弹宽度的一半，使其居中
            bullet_pos.y,
            5.0,
            20.0,
            GameObjectType::Bullet,
        )?;
        self.bullets.push(bullet);
        Ok(())
    }

    fn update_window_size(&mut self, ctx: &mut ggez::Context) {
        let window = ctx.gfx.window();
        let new_size = window.inner_size();
        self.window_size = WindowSize::new(
            new_size.width as f32,
            new_size.height as f32,
        );
    }
}

impl EventHandler for MainState {
    fn update(&mut self, ctx: &mut ggez::Context) -> GameResult {
        self.update_window_size(ctx);

        if self.game_over {
            return Ok(());
        }

        let mut dx = 0.0;
        let mut dy = 0.0;

        let player_speed = PLAYER_SPEED_RATIO * self.window_size.width;

        if keyboard::is_key_pressed(ctx, KeyCode::Left) || keyboard::is_key_pressed(ctx, KeyCode::A) {
            dx -= player_speed;
        }
        if keyboard::is_key_pressed(ctx, KeyCode::Right) || keyboard::is_key_pressed(ctx, KeyCode::D) {
            dx += player_speed;
        }
        if keyboard::is_key_pressed(ctx, KeyCode::Up) || keyboard::is_key_pressed(ctx, KeyCode::W) {
            dy -= player_speed;
        }
        if keyboard::is_key_pressed(ctx, KeyCode::Down) || keyboard::is_key_pressed(ctx, KeyCode::S) {
            dy += player_speed;
        }

        self.player.pos.x = (self.player.pos.x + dx)
            .clamp(0.0, BASE_WINDOW_WIDTH - self.player.base_size.x);
        self.player.pos.y = (self.player.pos.y + dy)
            .clamp(0.0, BASE_WINDOW_HEIGHT - self.player.base_size.y);

        self.shoot_cooldown = self.shoot_cooldown.saturating_sub(ctx.time.delta());

        if keyboard::is_key_pressed(ctx, KeyCode::Space) && self.shoot_cooldown.is_zero() {
            self.shoot(ctx)?;
            self.shoot_cooldown = Duration::from_millis(250);
        }

        let bullet_speed = BULLET_SPEED_RATIO * self.window_size.height;
        for bullet in &mut self.bullets {
            bullet.pos.y -= bullet_speed;
        }
        self.bullets.retain(|bullet| bullet.pos.y > -bullet.base_size.y);

        self.spawn_timer += ctx.time.delta();
        if self.spawn_timer.as_secs_f32() >= 1.0 {
            self.spawn_enemy(ctx)?;
            self.spawn_timer = Duration::from_secs(0);
        }

        let enemy_speed = ENEMY_SPEED_RATIO * self.window_size.height;
        for enemy in &mut self.enemies {
            enemy.pos.y += enemy_speed;
            if enemy.intersects(&self.player, &self.window_size) {
                self.game_over = true;
            }
        }
        self.enemies.retain(|enemy| enemy.pos.y < BASE_WINDOW_HEIGHT);

        // 更新星空
        for (pos, _) in &mut self.star_field {
            pos.y += 0.5 * self.window_size.scale_y;
            if pos.y > BASE_WINDOW_HEIGHT {
                pos.y = 0.0;
            }
        }

        // 碰撞检测和爆炸效果
        let mut destroyed_bullets = HashSet::new();
        let mut destroyed_enemies = HashSet::new();
        let mut explosion_positions = Vec::new();

        for (bullet_idx, bullet) in self.bullets.iter().enumerate() {
            for (enemy_idx, enemy) in self.enemies.iter().enumerate() {
                if !destroyed_bullets.contains(&bullet_idx) &&
                    !destroyed_enemies.contains(&enemy_idx) &&
                    bullet.intersects(enemy, &self.window_size) {
                    destroyed_bullets.insert(bullet_idx);
                    destroyed_enemies.insert(enemy_idx);
                    self.score += 10;

                    self.sounds.play_explosion(ctx)?;

                    explosion_positions.push((
                        enemy.pos + enemy.base_size * 0.5,
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

        // 创建爆炸效果
        for (pos, color) in explosion_positions {
            self.particles.add_explosion(pos, color, &self.window_size);
        }

        // 更新粒子系统
        self.particles.update(ctx.time.delta().as_secs_f32(), &self.window_size);

        Ok(())
    }

    fn draw(&mut self, ctx: &mut ggez::Context) -> GameResult {
        let mut canvas = Canvas::from_frame(ctx, Color::new(0.0, 0.05, 0.1, 1.0));

        // 绘制星空
        for (pos, size) in &self.star_field {
            let scaled_pos = self.window_size.scale_vec2(*pos);
            let scaled_size = size * self.window_size.scale_x.min(self.window_size.scale_y);

            let star = Mesh::new_circle(
                ctx,
                graphics::DrawMode::fill(),
                scaled_pos,
                scaled_size,
                0.1,
                Color::WHITE,
            )?;
            canvas.draw(&star, DrawParam::default());
        }

        // 绘制游戏对象
        self.player.draw(&mut canvas, &self.window_size);

        for bullet in &self.bullets {
            bullet.draw(&mut canvas, &self.window_size);
        }

        for enemy in &self.enemies {
            enemy.draw(&mut canvas, &self.window_size);
        }

        // 绘制粒子效果
        self.particles.draw(ctx, &mut canvas, &self.window_size)?;

        // 绘制分数
        let score_text = graphics::Text::new(format!("Score: {}", self.score));
        let score_pos = self.window_size.scale_vec2(Vec2::new(10.0, 10.0));
        canvas.draw(
            &score_text,
            DrawParam::default()
                .dest(score_pos)
                .color(Color::WHITE)
                .scale(Vec2::new(
                    self.window_size.scale_x,
                    self.window_size.scale_y
                ))
        );

        // 绘制游戏结束提示
        if self.game_over {
            let game_over_text = graphics::Text::new("Game Over!");
            let text_pos = self.window_size.scale_vec2(Vec2::new(
                BASE_WINDOW_WIDTH/2.0 - 100.0,
                BASE_WINDOW_HEIGHT/2.0
            ));
            canvas.draw(
                &game_over_text,
                DrawParam::default()
                    .dest(text_pos)
                    .color(Color::RED)
                    .scale(Vec2::new(
                        self.window_size.scale_x * 2.0,
                        self.window_size.scale_y * 2.0
                    ))
            );
        }

        canvas.finish(ctx)?;
        Ok(())
    }

    fn resize_event(&mut self, ctx: &mut ggez::Context, width: f32, height: f32) -> GameResult {
        self.window_size = WindowSize::new(width, height);
        Ok(())
    }
}

fn main() -> GameResult {
    // 设置资源目录
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let mut path = path::PathBuf::from(manifest_dir);
        path.push(RESOURCE_DIR);
        println!("Adding resource path: {:?}", path);
        env::set_var("CARGO_RESOURCE_ROOT", path);
    }

    // 创建游戏上下文
    let cb = ggez::ContextBuilder::new("vertical_shooter", "author")
        .window_setup(ggez::conf::WindowSetup::default()
            .title("Vertical Shooter")
            .vsync(true))
        .window_mode(ggez::conf::WindowMode::default()
            .dimensions(BASE_WINDOW_WIDTH, BASE_WINDOW_HEIGHT)
            .resizable(true)
            .min_dimensions(400.0, 300.0))  // 设置最小窗口尺寸
        .add_resource_path(path::PathBuf::from(RESOURCE_DIR));

    // 构建游戏并运行
    let (mut ctx, event_loop) = cb.build()?;
    let state = MainState::new(&mut ctx)?;
    event::run(ctx, event_loop, state)
}