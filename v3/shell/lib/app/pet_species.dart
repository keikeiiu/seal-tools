/// The pet species the game data knows (the `species` column of v3/docs/pet-data.csv).
///
/// The Pet tab uses this as a dropdown instead of making the player type a feeding-table line:
/// the value comes from the game's own knowledge and the run's growth→minutes rule relies on it.
library;

const List<String> petSpecies = [
  '三尾狐',
  '咕咕类',
  '天蛋类',
  '异色咕咕',
  '异色天蛋',
  '异色种子',
  '异色鸟蛋',
  '曼德拉',
  '种子类',
  '鸟蛋类',
  '黑龙类',
];
