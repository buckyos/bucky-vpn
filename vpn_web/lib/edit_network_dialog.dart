import 'package:flutter/material.dart';

class EditNetworkDialog extends StatefulWidget {
  final String? name;
  final String? address;
  final int? mask;
  final Function(String, String, int) onSave;
  const EditNetworkDialog({super.key, this.name, this.address, this.mask, required this.onSave});

  @override
  createState() => _EditNetworkDialogState();
}

class _EditNetworkDialogState extends State<EditNetworkDialog> {
  late TextEditingController _controllerName;
  late TextEditingController _controllerAddress;
  late TextEditingController _controllerMask;

  @override
  void initState() {
    super.initState();
    _controllerName = TextEditingController(text: widget.name ?? "");
    _controllerAddress = TextEditingController(text: widget.address ?? "");
    _controllerMask = TextEditingController(text: widget.mask?.toString() ?? "");
  }

  @override
  Widget build(BuildContext context) {
    return Dialog(
      child: Container(
        width: 400,
        height: 300,
        padding: EdgeInsets.only(left: 30.0, top: 50, right: 30, bottom: 50),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: <Widget>[
            TextField(
              controller: _controllerName,
              decoration: const InputDecoration(
                labelText: 'Network Name',
              ),
            ),
            TextField(
              controller: _controllerAddress,
              decoration: const InputDecoration(
                labelText: 'Network Address',
              ),
            ),
            TextField(
              controller: _controllerMask,
              decoration: const InputDecoration(
                labelText: 'Network Mask',
              ),
            ),
            const SizedBox(height: 20),
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceEvenly,
              children: [
                ElevatedButton(
                  onPressed: () {
                    Navigator.of(context).pop();
                    if (_controllerName.text.isNotEmpty && _controllerAddress.text.isNotEmpty && _controllerMask.text.isNotEmpty) {
                      widget.onSave(_controllerName.text, _controllerAddress.text, int.parse(_controllerMask.text));
                    }
                  },
                  child: const Text('Save'),
                ),
                ElevatedButton(
                  onPressed: () {
                    Navigator.of(context).pop();
                  },
                  child: const Text('Cancel'),
                )
              ],
            )
            ,
          ],
        ),
      ),
    );
  }

}
