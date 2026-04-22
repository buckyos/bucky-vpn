const List<String> _trafficUnits = ['B', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB'];

String formatTrafficBytes(String rawValue) {
  return _formatTrafficValue(rawValue, perSecond: false);
}

String formatTrafficSpeed(String rawValue) {
  return _formatTrafficValue(rawValue, perSecond: true);
}

String _formatTrafficValue(String rawValue, {required bool perSecond}) {
  final value = BigInt.tryParse(rawValue) ?? BigInt.zero;
  final base = BigInt.from(1024);
  var unitIndex = 0;
  var unitDivisor = BigInt.one;

  while (unitIndex < _trafficUnits.length - 1 && value >= unitDivisor * base) {
    unitDivisor *= base;
    unitIndex += 1;
  }

  final whole = value ~/ unitDivisor;
  final suffix =
      perSecond ? '${_trafficUnits[unitIndex]}/s' : _trafficUnits[unitIndex];

  if (unitIndex == 0) {
    return '${whole.toString()} $suffix';
  }

  final fraction = ((value % unitDivisor) * BigInt.from(10)) ~/ unitDivisor;
  final shouldShowFraction = whole < BigInt.from(10) && fraction > BigInt.zero;
  final displayValue = shouldShowFraction
      ? '${whole.toString()}.${fraction.toString()}'
      : whole.toString();
  return '$displayValue $suffix';
}
