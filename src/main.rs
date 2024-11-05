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
    GuidedMissile,
    MissileAmmo,  // 新增：导弹弹药补给
    SpreadShot,     // 新增：扇形子弹
    SpreadAmmo,     // 新增：扇形弹药
}

// 游戏对象结构体
struct GameObject {
    pos: Vec2,
    base_size: Vec2,
    speed: Vec2,
    image: Option<Image>,
    rotation: f32,
    object_type: GameObjectType,
    target: Option<usize>,  // 新增：用于存储目标敌人的索引
}

impl GameObject {
    fn new(ctx: &mut ggez::Context, x: f32, y: f32, width: f32, height: f32, object_type: GameObjectType) -> GameResult<Self> {
        let (image, rotation) = match object_type {
            GameObjectType::Player => (Some(Image::from_path(ctx, "/img/player.png")?), 0.0),
            GameObjectType::Bullet => (Some(Image::from_path(ctx, "/img/bullet.png")?), 0.0),
            GameObjectType::Enemy => (Some(Image::from_path(ctx, "/img/player.png")?), std::f32::consts::PI),
            GameObjectType::GuidedMissile => (Some(Image::from_path(ctx, "/img/bullet.png")?), 0.0),  // 使用子弹图片
            GameObjectType::MissileAmmo => (Some(Image::from_path(ctx, "/img/bullet.png")?), 0.0),  // 暂时使用子弹图片
            GameObjectType::SpreadShot => (Some(Image::from_path(ctx, "/img/bullet.png")?), 0.0),  // 使用子弹图片
            GameObjectType::SpreadAmmo => (Some(Image::from_path(ctx, "/img/bullet.png")?), 0.0),  // 使用子弹图片


        };

        Ok(GameObject {
            pos: Vec2::new(x, y),
            base_size: Vec2::new(width, height),
            speed: Vec2::ZERO,
            image,
            rotation,
            object_type,
            target: None,
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
                    .offset(Vec2::new(0.5, 0.5))  // 这里使用了 0.5 offset，意味着旋转中心在图片中心
                    .scale(Vec2::new(
                        scaled_size.x / image.width() as f32,
                        scaled_size.y / image.height() as f32
                    ))
            );
        }
    }

    // 添加导弹追踪逻辑
    fn update_guided_missile(&mut self, enemies: &Vec<GameObject>, window_size: &WindowSize) {
        const MISSILE_SPEED: f32 = 4.0;  // 导弹基础速度
        const TURN_RATE: f32 = 0.1;      // 转向速率

        if let Some(target_idx) = self.target {
            if target_idx < enemies.len() {
                let target = &enemies[target_idx];
                let direction = target.pos - self.pos;
                let distance = direction.length();

                if distance > 0.0 {
                    // 计算目标角度
                    let target_angle = direction.y.atan2(direction.x);

                    // 平滑转向
                    let angle_diff = target_angle - self.rotation;
                    let angle_diff = if angle_diff > std::f32::consts::PI {
                        angle_diff - 2.0 * std::f32::consts::PI
                    } else if angle_diff < -std::f32::consts::PI {
                        angle_diff + 2.0 * std::f32::consts::PI
                    } else {
                        angle_diff
                    };

                    self.rotation += angle_diff * TURN_RATE;

                    // 更新速度
                    self.speed.x = self.rotation.cos() * MISSILE_SPEED * window_size.scale_x;
                    self.speed.y = self.rotation.sin() * MISSILE_SPEED * window_size.scale_y;
                }
            }
        }
    }


    // 添加一个新方法来绘制碰撞范围
    fn draw_collision_circle(&self, ctx: &mut ggez::Context, canvas: &mut Canvas, window_size: &WindowSize) -> GameResult {
        let center = self.pos;

        // 在 draw_collision_circle 中的半径设置
        let radius = match self.object_type {
            GameObjectType::Bullet => self.base_size.x * 0.8,      // 匹配碰撞检测逻辑
            GameObjectType::Enemy => self.base_size.x * 0.45,      // 匹配碰撞检测逻辑
            GameObjectType::Player => self.base_size.x * 0.4,      // 保持不变
            GameObjectType::GuidedMissile => self.base_size.x * 1.0, // 导弹的碰撞范围稍大
            GameObjectType::MissileAmmo => self.base_size.x * 0.6,   // 弹药包的碰撞范围
            GameObjectType::SpreadShot => self.base_size.x * 0.8,    // 与普通子弹相同
            GameObjectType::SpreadAmmo => self.base_size.x * 0.6,    // 与普通弹药包相同
        };


        let scaled_center = window_size.scale_vec2(center);
        let scaled_radius = radius * window_size.scale_x.min(window_size.scale_y);

        // 在 draw_collision_circle 中的颜色设置
        let color = match self.object_type {
            GameObjectType::Bullet => Color::new(1.0, 1.0, 0.0, 0.5),    // 黄色
            GameObjectType::Enemy => Color::new(1.0, 0.0, 0.0, 0.5),     // 红色
            GameObjectType::Player => Color::new(0.0, 1.0, 0.0, 0.5),    // 绿色
            GameObjectType::GuidedMissile => Color::new(1.0, 0.0, 1.0, 0.5), // 紫色
            GameObjectType::MissileAmmo => Color::new(0.0, 1.0, 1.0, 0.5),   // 青色
            GameObjectType::SpreadShot => Color::new(1.0, 0.5, 0.0, 0.5),    // 橙色
            GameObjectType::SpreadAmmo => Color::new(1.0, 0.5, 0.0, 0.5),    // 橙色
        };

        let circle = Mesh::new_circle(
            ctx,
            graphics::DrawMode::stroke(2.0),
            [scaled_center.x, scaled_center.y],
            scaled_radius,
            0.1,
            color,
        )?;

        canvas.draw(&circle, DrawParam::default());
        Ok(())
    }

    fn intersects(&self, other: &GameObject, window_size: &WindowSize) -> bool {
        let self_center = self.pos;
        let other_center = other.pos;

        // 专门处理子弹和敌机的碰撞
        let (self_radius, other_radius) = match (&self.object_type, &other.object_type) {
            // 子弹打敌机的情况
            (GameObjectType::Bullet, GameObjectType::Enemy) |
            (GameObjectType::SpreadShot, GameObjectType::Enemy) => {
                let bullet_radius = self.base_size.x * 0.8;
                let enemy_radius = other.base_size.x * 0.45;
                (bullet_radius, enemy_radius)
            },
            // 敌机被子弹打的情况
            (GameObjectType::Enemy, GameObjectType::Bullet) |
            (GameObjectType::Enemy, GameObjectType::SpreadShot) => {
                let enemy_radius = self.base_size.x * 0.45;
                let bullet_radius = other.base_size.x * 0.8;
                (enemy_radius, bullet_radius)
            },
            // 玩家和弹药包的碰撞
            (GameObjectType::Player, GameObjectType::MissileAmmo) |
            (GameObjectType::Player, GameObjectType::SpreadAmmo) |
            (GameObjectType::MissileAmmo, GameObjectType::Player) |
            (GameObjectType::SpreadAmmo, GameObjectType::Player) => {
                let radius = self.base_size.x.min(self.base_size.y) * 0.6;
                (radius, radius)
            },
            // 玩家和敌机的碰撞
            (GameObjectType::Player, GameObjectType::Enemy) |
            (GameObjectType::Enemy, GameObjectType::Player) => {
                let radius = self.base_size.x.min(self.base_size.y) * 0.4;
                (radius, radius)
            },
            // 其他情况
            _ => {
                let radius = self.base_size.x.min(self.base_size.y) * 0.4;
                (radius, radius)
            }
        };

        // 计算实际的碰撞距离
        let scaled_self_center = window_size.scale_vec2(self_center);
        let scaled_other_center = window_size.scale_vec2(other_center);
        let scaled_self_radius = self_radius * window_size.scale_x.min(window_size.scale_y);
        let scaled_other_radius = other_radius * window_size.scale_x.min(window_size.scale_y);

        // 计算中心点距离并判断是否碰撞
        let distance = scaled_self_center.distance(scaled_other_center);
        distance < (scaled_self_radius + scaled_other_radius)
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
    paused: bool,    // 新增：暂停状态
    shoot_cooldown: Duration,
    star_field: Vec<(Vec2, f32)>,
    particles: ParticleSystem,
    sounds: SoundEffects,
    missile_cooldown: Duration,  // 新增：导弹冷却时间
    missile_ammo: i32,           // 新增：当前导弹数量
    ammo_spawn_timer: Duration,  // 新增：弹药生成计时器
    ammo_items: Vec<GameObject>, // 新增：场景中的弹药
    p_key_pressed: bool,  // 新增：追踪 P 键状态
    has_spread_shot: bool,  // 新增：是否拥有扇形射击能力
}

impl MainState {
    fn new(ctx: &mut ggez::Context) -> GameResult<MainState> {
        let window_size = WindowSize::new(BASE_WINDOW_WIDTH, BASE_WINDOW_HEIGHT);

        // 修改玩家初始位置，考虑到中心点定位
        let player = GameObject::new(
            ctx,
            BASE_WINDOW_WIDTH / 2.0,  // 水平居中
            BASE_WINDOW_HEIGHT - 30.0, // 距离底部一定距离
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
            paused: false,    // 初始化暂停状态为 false
            shoot_cooldown: Duration::from_secs(0),
            star_field,
            particles: ParticleSystem::new(),
            sounds,
            missile_cooldown: Duration::from_secs(0),
            missile_ammo: 5,              // 初始5发导弹
            ammo_spawn_timer: Duration::from_secs(0),
            ammo_items: Vec::new(),
            p_key_pressed: false,  // 初始化为 false
            has_spread_shot: false,
        })

    }

    // 添加游戏重置方法
    fn reset(&mut self, ctx: &mut ggez::Context) -> GameResult {
        self.player = GameObject::new(
            ctx,
            BASE_WINDOW_WIDTH / 2.0,
            BASE_WINDOW_HEIGHT - 30.0,
            50.0,
            60.0,
            GameObjectType::Player,
        )?;

        self.bullets.clear();
        self.enemies.clear();
        self.ammo_items.clear();
        self.score = 0;
        self.game_over = false;
        self.paused = false;
        self.spawn_timer = Duration::from_secs(0);
        self.shoot_cooldown = Duration::from_secs(0);
        self.missile_cooldown = Duration::from_secs(0);
        self.missile_ammo = 5;
        self.ammo_spawn_timer = Duration::from_secs(0);
        self.p_key_pressed = false;
        self.has_spread_shot = false;
        Ok(())
    }

    // 添加扇形弹药生成方法
    fn spawn_spread_ammo(&mut self, ctx: &mut ggez::Context) -> GameResult {
        let mut rng = rand::thread_rng();
        let x = rng.gen_range(0.0..BASE_WINDOW_WIDTH - 20.0);

        let ammo = GameObject::new(
            ctx,
            x,
            -30.0,
            25.0,  // 稍微大一点
            25.0,
            GameObjectType::SpreadAmmo,
        )?;

        self.ammo_items.push(ammo);
        Ok(())
    }

    // 添加生成弹药的方法
    fn spawn_missile_ammo(&mut self, ctx: &mut ggez::Context) -> GameResult {
        let mut rng = rand::thread_rng();
        let x = rng.gen_range(0.0..BASE_WINDOW_WIDTH - 20.0);

        let ammo = GameObject::new(
            ctx,
            x,
            -30.0,
            20.0,  // 弹药包大小
            20.0,
            GameObjectType::MissileAmmo,
        )?;

        self.ammo_items.push(ammo);
        Ok(())
    }

    // 添加发射导弹的方法
    fn launch_missile(&mut self, ctx: &mut ggez::Context) -> GameResult {
        if self.enemies.is_empty() || self.missile_ammo <= 0 {
            return Ok(());  // 如果没有敌人或没有导弹，不发射
        }

        // 找到最近的敌人
        let player_pos = self.player.pos;
        let mut closest_enemy = 0;
        let mut min_distance = f32::MAX;

        for (idx, enemy) in self.enemies.iter().enumerate() {
            let distance = enemy.pos.distance(player_pos);
            if distance < min_distance {
                min_distance = distance;
                closest_enemy = idx;
            }
        }

        // 创建导弹并设置目标
        let mut missile = GameObject::new(
            ctx,
            self.player.pos.x,
            self.player.pos.y - self.player.base_size.y / 2.0,
            8.0,  // 稍微大一点的尺寸
            24.0,
            GameObjectType::GuidedMissile,
        )?;
        missile.target = Some(closest_enemy);

        self.bullets.push(missile);
        self.sounds.play_shoot(ctx)?;

        // 发射后减少弹药
        self.missile_ammo -= 1;
        Ok(())
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

    // 修改射击方法添加扇形射击
    fn shoot(&mut self, ctx: &mut ggez::Context) -> GameResult {
        self.sounds.play_shoot(ctx)?;

        let center_x = self.player.pos.x;
        let top_y = self.player.pos.y - self.player.base_size.y / 2.0;
        let bullet_pos = Vec2::new(center_x, top_y);

        // 添加粒子效果
        self.particles.add_explosion(
            bullet_pos,
            if self.has_spread_shot {
                Color::new(1.0, 0.5, 0.0, 0.5)  // 橙色
            } else {
                Color::new(1.0, 1.0, 0.0, 0.5)  // 黄色
            },
            &self.window_size,
        );

        if self.has_spread_shot {
            // 扇形射击：发射5发子弹，角度范围为60度
            let angles:[f32; 5] = [-30.0, -15.0, 0.0, 15.0, 30.0];  // 角度（度）
            for &angle in angles.iter() {
                let rad: f32 = angle.to_radians();
                let direction = Vec2::new(rad.sin(), -rad.cos());
                let mut bullet = GameObject::new(
                    ctx,
                    bullet_pos.x,
                    bullet_pos.y,
                    5.0,
                    20.0,
                    GameObjectType::SpreadShot,
                )?;
                bullet.speed = direction * BULLET_SPEED_RATIO * self.window_size.height;
                bullet.rotation = rad;  // 设置子弹旋转角度
                self.bullets.push(bullet);
            }
        } else {
            // 普通射击
            let bullet = GameObject::new(
                ctx,
                bullet_pos.x - 2.5,
                bullet_pos.y,
                5.0,
                20.0,
                GameObjectType::Bullet,
            )?;
            self.bullets.push(bullet);
        }

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

        // 处理暂停键
        if keyboard::is_key_pressed(ctx, KeyCode::P) {
            if !self.p_key_pressed {  // 只在按键首次按下时触发
                self.paused = !self.paused;
                self.p_key_pressed = true;
            }
        } else {
            self.p_key_pressed = false;  // 当按键释放时重置状态
        }

        //重新开始
        if self.game_over {
            if keyboard::is_key_pressed(ctx, KeyCode::Space) {
                self.reset(ctx)?;
            }
            return Ok(());
        }

        // 如果游戏暂停，只处理继续游戏的输入
        if self.paused {
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

        // 更新导弹冷却时间
        self.missile_cooldown = self.missile_cooldown.saturating_sub(ctx.time.delta());

        // 处理发射追踪导弹
        if keyboard::is_key_pressed(ctx, KeyCode::X) && self.missile_cooldown.is_zero() {
            self.launch_missile(ctx)?;
            self.missile_cooldown = Duration::from_millis(1000);  // 1秒冷却时间
        }

        // 在子弹更新逻辑中添加扇形子弹的处理
        let bullet_speed = BULLET_SPEED_RATIO * self.window_size.height;
        for bullet in &mut self.bullets {
            match bullet.object_type {
                GameObjectType::Bullet => {
                    bullet.pos.y -= bullet_speed;
                }
                GameObjectType::SpreadShot => {
                    bullet.pos += bullet.speed;  // 使用预设的速度和方向
                }
                GameObjectType::GuidedMissile => {
                    bullet.update_guided_missile(&self.enemies, &self.window_size);
                    bullet.pos += bullet.speed;
                }
                _ => {}
            }
        }

        // 在弹药生成逻辑中随机生成扇形弹药
        self.ammo_spawn_timer += ctx.time.delta();
        if self.ammo_spawn_timer.as_secs_f32() >= 15.0 {
            if rand::random::<bool>() {  // 50%概率生成普通导弹弹药或扇形弹药
                self.spawn_missile_ammo(ctx)?;
            } else {
                self.spawn_spread_ammo(ctx)?;
            }
            self.ammo_spawn_timer = Duration::from_secs(0);
        }


        self.bullets.retain(|bullet| bullet.pos.y > -bullet.base_size.y);

        // 处理敌人生成
        self.spawn_timer += ctx.time.delta();
        if self.spawn_timer.as_secs_f32() >= 1.0 {
            self.spawn_enemy(ctx)?;
            self.spawn_timer = Duration::from_secs(0);
        }

        // 更新敌人位置
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
                    // 导弹击中给更多分数
                    self.score += match bullet.object_type {
                        GameObjectType::GuidedMissile => 20,
                        _ => 10,
                    };

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


        // 更新弹药生成计时器
        // self.ammo_spawn_timer += ctx.time.delta();
        // if self.ammo_spawn_timer.as_secs_f32() >= 15.0 { // 每15秒生成一个弹药包
        //     self.spawn_missile_ammo(ctx)?;
        //     self.ammo_spawn_timer = Duration::from_secs(0);
        // }

        // 更新弹药位置
        let ammo_speed = ENEMY_SPEED_RATIO * self.window_size.height;
        for ammo in &mut self.ammo_items {
            ammo.pos.y += ammo_speed;
        }
        self.ammo_items.retain(|ammo| ammo.pos.y < BASE_WINDOW_HEIGHT);

        // 检测玩家与弹药的碰撞
        let mut collected_ammo = Vec::new();
        for (idx, ammo) in self.ammo_items.iter().enumerate() {
            if ammo.intersects(&self.player, &self.window_size) {
                collected_ammo.push(idx);
                self.missile_ammo += 3; // 每个弹药包补充3发导弹

                // 添加收集效果
                self.particles.add_explosion(
                    ammo.pos,
                    Color::new(0.0, 1.0, 1.0, 1.0), // 青色粒子效果
                    &self.window_size,
                );
            }
        }

        // 移除被收集的弹药
        for idx in collected_ammo.iter().rev() {
            self.ammo_items.remove(*idx);
        }

        // 修改弹药拾取逻辑，确保正确处理所有类型的弹药
        let mut collected_ammo = Vec::new();
        for (idx, ammo) in self.ammo_items.iter().enumerate() {
            if ammo.intersects(&self.player, &self.window_size) {
                collected_ammo.push(idx);
                match ammo.object_type {
                    GameObjectType::SpreadAmmo => {
                        self.has_spread_shot = true;
                        self.particles.add_explosion(
                            ammo.pos,
                            Color::new(1.0, 0.5, 0.0, 1.0), // 橙色粒子效果
                            &self.window_size,
                        );
                    }
                    GameObjectType::MissileAmmo => {
                        self.missile_ammo += 3; // 每个弹药包补充3发导弹
                        self.particles.add_explosion(
                            ammo.pos,
                            Color::new(0.0, 1.0, 1.0, 1.0), // 青色粒子效果
                            &self.window_size,
                        );
                    }
                    _ => {}
                }
            }
        }

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

        // 绘制弹药和碰撞圈
        for ammo in &self.ammo_items {
            ammo.draw(&mut canvas, &self.window_size);
            ammo.draw_collision_circle(ctx, &mut canvas, &self.window_size)?;  // 添加碰撞圈显示
        }

        for bullet in &self.bullets {
            bullet.draw(&mut canvas, &self.window_size);
            bullet.draw_collision_circle(ctx, &mut canvas, &self.window_size)?;
        }

        for enemy in &self.enemies {
            enemy.draw(&mut canvas, &self.window_size);
        }

        // 绘制导弹数量和扇形状态
        let ammo_text = graphics::Text::new(format!("Missiles: {}", self.missile_ammo));
        let ammo_pos = self.window_size.scale_vec2(Vec2::new(10.0, 40.0));
        canvas.draw(
            &ammo_text,
            DrawParam::default()
                .dest(ammo_pos)
                .color(Color::WHITE)
                .scale(Vec2::new(
                    self.window_size.scale_x,
                    self.window_size.scale_y
                ))
        );

        // 绘制扇形弹药状态
        let spread_text = graphics::Text::new(
            if self.has_spread_shot {
                "Spread Shot: Active"
            } else {
                "Spread Shot: -"
            }
        );
        let spread_pos = self.window_size.scale_vec2(Vec2::new(10.0, 70.0));
        canvas.draw(
            &spread_text,
            DrawParam::default()
                .dest(spread_pos)
                .color(if self.has_spread_shot {
                    Color::new(1.0, 0.5, 0.0, 1.0) // 橙色
                } else {
                    Color::new(0.5, 0.5, 0.5, 1.0) // 灰色
                })
                .scale(Vec2::new(
                    self.window_size.scale_x,
                    self.window_size.scale_y
                ))
        );

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

        // 绘制粒子效果
        self.particles.draw(ctx, &mut canvas, &self.window_size)?;

        // 绘制游戏结束和暂停提示
        if self.game_over {
            let game_over_text = graphics::Text::new("Game Over!\nPress SPACE to restart");
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

        if self.paused {
            let pause_text = graphics::Text::new("PAUSED\nPress P to continue");
            let text_pos = self.window_size.scale_vec2(Vec2::new(
                BASE_WINDOW_WIDTH/2.0 - 100.0,
                BASE_WINDOW_HEIGHT/2.0
            ));
            canvas.draw(
                &pause_text,
                DrawParam::default()
                    .dest(text_pos)
                    .color(Color::YELLOW)
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